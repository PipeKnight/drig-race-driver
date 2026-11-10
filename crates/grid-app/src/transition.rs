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
//! (FUN_004A4560). The circle is the procedural shader `feov_ramp.fx`:
//! the material's colours and its animated ramp window are the original's
//! data, the ring's mesh and size are AUTHORED (see [`ramp_share`]). Front-end
//! screens show the overlay's letterbox bars ([`borders`]).
//!
//! Each screen is its own process (winit allows one event loop per
//! process). Under the supervisor (`event_menu::supervise`) a leaving screen
//! stays black with the loading overlay until the next process shows its
//! first frame, so no desktop shows between them: see [`run_supervised`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
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
/// The loading ring's texture, redrawn each frame while loading.
#[derive(Component)]
struct RingImage(Handle<Image>);
#[derive(Component)]
struct Bars;
#[derive(Component)]
struct Bar {
    top: bool,
}

// ---- The overlay's bars ----------------------------------------------------

/// The overlay's letterbox bars, `{2}null=topbar` and `{2}null=bottombar` in
/// `fePersistentDatabase.pssg`: black quads (`overlay_black`) 200 units deep
/// that animation `Anim!160` slides in (frame 0) and out (frame 0.6667, the
/// overlay's default frame). Translation Y keys at frames 0, 0.3333, 0.6667
/// [D]; linear between them [I].
const TOP_BAR_Y: [(f32, f32); 3] = [(0., 852.123), (0.3333, 882.123), (0.6667, 1002.015)];
const BOTTOM_BAR_Y: [(f32, f32); 3] = [(0., -26.122), (0.3333, -59.122), (0.6667, -172.521)];
const BAR_DEPTH: f32 = 200.;
/// The ortho camera `camera=overlay`: half-height 475.861 round the point
/// its view axis meets z = 0, its target `target=overlay` (y 307.908; the
/// camera at y 341.639 looks 1.34 degrees down onto it) [D].
const VIEW_CENTRE_Y: f32 = 307.908;
const VIEW_HALF_HEIGHT: f32 = 475.861;
/// `border_off` plays to this frame; `border_on` to 0 (0.3 s each).
const BARS_OFF: f32 = 0.6667;
const BORDER_SECONDS: f32 = 0.3;

/// A value keyed over time: linear between the keys, held outside them.
fn keyed(keys: &[(f32, f32)], t: f32) -> f32 {
    let (first, last) = (keys[0], keys[keys.len() - 1]);
    if t <= first.0 {
        return first.1;
    }
    for w in keys.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0);
        }
    }
    last.1
}

/// The shares of the window height the top and bottom bar cover at an
/// animation frame (0: on, 0.6667: off).
pub fn bar_shares(frame: f32) -> (f32, f32) {
    let (top, bottom) = (VIEW_CENTRE_Y + VIEW_HALF_HEIGHT, VIEW_CENTRE_Y - VIEW_HALF_HEIGHT);
    let height = 2. * VIEW_HALF_HEIGHT;
    let top_bar = (top - (keyed(&TOP_BAR_Y, frame) - BAR_DEPTH)).clamp(0., height);
    let bottom_bar = (keyed(&BOTTOM_BAR_Y, frame) - bottom).clamp(0., height);
    (top_bar / height, bottom_bar / height)
}

/// The bars' state, on a screen that shows them ([`borders`]). They are on
/// when the screen opens (the overlay's set-up posts `border_on`,
/// FUN_004A49E0, through FUN_004964C0) and slide off once the screen is
/// black on the way out (`border_off`, FUN_00496770):
/// the loading text shows when `border_off` finishes (FUN_004A4560).
#[derive(Resource, Debug)]
pub struct Borders {
    from: f32,
    to: f32,
    elapsed: f32,
}

impl Borders {
    fn on() -> Self {
        Self { from: 0., to: 0., elapsed: BORDER_SECONDS }
    }

    pub fn frame(&self) -> f32 {
        self.from + (self.to - self.from) * (self.elapsed / BORDER_SECONDS).min(1.)
    }

    /// `border_off` (once).
    fn slide_off(&mut self) {
        if self.to < BARS_OFF {
            self.from = self.frame();
            self.to = BARS_OFF;
            self.elapsed = 0.;
            log_line!("screen_transition version=1 event=border_off seconds={BORDER_SECONDS:.2}");
        }
    }

    pub fn off(&self) -> bool {
        self.to >= BARS_OFF && self.elapsed >= BORDER_SECONDS
    }

    fn advance(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt.min(MAX_STEP)).min(BORDER_SECONDS);
    }
}

/// Shows the overlay's letterbox bars on a front-end screen (after
/// [`install`]). The original's screens draw their button help on the
/// bottom bar (`component/overlay_helptext` is its child); the race has
/// them only in its intro, which DRIG does not have (`border_off` at the
/// hand-over to the in-game cameras, FUN_0058BAF0).
pub fn borders(app: &mut App) {
    app.insert_resource(Borders::on());
}

/// Installs the overlay on a windowed app: the screen starts black and
/// `start` says when it clears. `loading` is the loading overlay's text.
pub fn install(app: &mut App, loading: &str, start: Start) {
    app.insert_resource(Fade::black(Some(start)))
        .insert_resource(LoadingText(loading.to_owned()))
        .add_systems(First, finish)
        .add_systems(Update, (advance, (spawn, present).chain()).chain());
    add_window_show(app);
}

/// `handoff_ready` before Last: bevy_winit's `changed_windows` (Last) must
/// see the shown window in the same frame. Applied a frame late, the window
/// stays hidden for good: a hidden window gets no RedrawRequested, so on
/// Windows no further update runs (F773, docs/SCREEN_FLOW.md §2.2).
fn add_window_show(app: &mut App) {
    app.add_systems(PostUpdate, handoff_ready);
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

fn advance(
    time: Res<Time<Real>>,
    mut fade: ResMut<Fade>,
    borders: Option<ResMut<Borders>>,
    warmup: Option<Res<crate::pipeline_warmup::Warmup>>,
) {
    fade.frames += 1;
    fade.advance(time.delta_secs());
    if let Some(mut borders) = borders {
        borders.advance(time.delta_secs());
    }
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
    mut own: Option<ResMut<SelfHandoff>>,
    borders: Option<ResMut<Borders>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(next) = fade.leaving else {
        return;
    };
    if !fade.is_black() {
        return;
    }
    // Black: the bars slide off, then the loading text shows.
    if let Some(mut borders) = borders {
        borders.slide_off();
        if !borders.off() {
            return;
        }
    }
    if !fade.loading {
        fade.loading = true;
        log_line!("screen_transition version=1 event=loading_on");
    }
    let go = std::env::var_os(GO_ENV).map(PathBuf::from);
    let supervised = go.zip(std::env::var_os(READY_ENV).map(PathBuf::from));
    let code = match next {
        Next::Code(code) if supervised.is_some() || own.is_some() => code,
        _ => {
            log_line!("screen_transition version=1 event=exit next={next:?}");
            exit_with(next, &mut exit);
            fade.leaving = None;
            return;
        }
    };
    if !fade.handed {
        fade.handed = true;
        let at = window.iter().next().and_then(|w| match w.position {
            WindowPosition::At(p) => Some(p),
            _ => None,
        });
        let handed = match (&supervised, own.as_mut()) {
            (Some((go, ready)), _) => {
                let _ = std::fs::remove_file(ready);
                let at = at.map(|p| format!(" {} {}", p.x, p.y)).unwrap_or_default();
                std::fs::write(go, format!("{code}{at}\n")).map_err(|e| e.to_string())
            }
            (None, Some(own)) => (own.spawn)(i32::from(code), at.map(|p| format!("{},{}", p.x, p.y)))
                .map(|ready| own.ready = Some(ready))
                .map_err(|e| e.to_string()),
            (None, None) => unreachable!("checked above"),
        };
        if let Err(error) = handed {
            warn!("hand-off failed, exiting at once: {error}");
            exit_with(next, &mut exit);
            fade.leaving = None;
            return;
        }
        allow_foreground();
        log_line!("screen_transition version=1 event=handoff code={code}");
        return;
    }
    let ready = match (supervised, own) {
        (Some((_, ready)), _) => Some(ready),
        (None, Some(own)) => own.ready.clone(),
        (None, None) => None,
    };
    let Some(ready) = ready else {
        exit_with(next, &mut exit);
        fade.leaving = None;
        return;
    };
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

/// A screen that starts its next process itself (a launcher's race, which
/// has no supervisor, opening the event menu): `spawn(code, at)` starts it
/// with the window position `X,Y` and returns the ready marker it writes.
/// The screen stays black with "Loading" until then, as under a supervisor.
#[derive(Resource)]
pub struct SelfHandoff {
    spawn: fn(i32, Option<String>) -> std::io::Result<PathBuf>,
    ready: Option<PathBuf>,
}

impl SelfHandoff {
    #[cfg(not(target_os = "android"))]
    pub fn new(spawn: fn(i32, Option<String>) -> std::io::Result<PathBuf>) -> Self {
        Self { spawn, ready: None }
    }
}

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
    borders: Option<Res<Borders>>,
    images: Option<ResMut<Assets<Image>>>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    let ring = images.map(|mut images| {
        let side = Extent3d {
            width: RING_TEXELS,
            height: RING_TEXELS,
            depth_or_array_layers: 1,
        };
        images.add(Image::new_fill(
            side,
            TextureDimension::D2,
            &[0; 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ))
    });
    if borders.is_some() {
        // Over the screen, under the fade (both black, so the order between
        // them does not show).
        commands
            .spawn((
                Bars,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.),
                    height: percent(100.),
                    ..default()
                },
                GlobalZIndex(i32::MAX - 2),
                UiTargetCamera(camera),
                Pickable::IGNORE,
            ))
            .with_children(|screen| {
                for top in [true, false] {
                    screen.spawn((
                        Bar { top },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.),
                            right: px(0.),
                            top: if top { px(0.) } else { Val::Auto },
                            bottom: if top { Val::Auto } else { px(0.) },
                            height: percent(0.),
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        Pickable::IGNORE,
                    ));
                }
            });
    }
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
                            width: px(RING_SIZE),
                            height: px(RING_SIZE),
                            ..default()
                        },
                    ))
                    .with_children(|circle| spawn_ring(circle, ring));
                });
        });
}

// ---- The loading circle ---------------------------------------------------

/// `load_circle_anim` (`Anim!164`, 3.333 s, which `circle_rotate` plays;
/// looped [I]): the ring material's `RampMin` and `RampMax` keys [D],
/// linear between them [I].
const RAMP_MIN: [(f32, f32); 3] = [(0., 0.), (1.6667, 0.), (3.3333, 1.2)];
const RAMP_MAX: [(f32, f32); 2] = [(0., -0.2), (1.6667, 1.)];
const RAMP_SOFTNESS: f32 = 0.2;
const CIRCLE_SECONDS: f32 = 3.3333;
/// The ring material `matinst=load_circle_anim` (feov_ramp.fx): grey 0.65
/// at alpha 0.5 inside the ramp window, `RampAlpha` 0 outside it [D].
const RING_GREY: f32 = 0.65;
const RING_ALPHA: f32 = 0.5;
/// The disk `matinst=load_circle_disk` (feov_colour.fx): yellow at alpha
/// 0.5 [D].
const DISK_COLOUR: [f32; 4] = [0.9272, 0.8058, 0.0312, 0.5];
/// The ring's geometry, AUTHORED: its mesh lives in a component instance
/// library the scene reader does not reach. Pixels.
const RING_SIZE: f32 = 28.;
const RING_WIDTH: f32 = 5.;
const DISK_SIZE: f32 = 10.;
/// The ring texture's side: twice the ring's size, for a smooth edge.
const RING_TEXELS: u32 = 56;

/// The share of the ring's alpha at ramp coordinate `u` (0 at the top to 1,
/// clockwise [I]) `t` seconds into the animation. [I] for the shader
/// (feov_ramp.fx is not decoded): a window from `RampMin` to `RampMax`
/// with soft edges `RampSoftness` wide outside it. The keys fit that
/// reading: `RampMax` starts one softness below 0 and `RampMin` ends one
/// above 1, so the ring starts and ends just empty and is just full at
/// 1.667 s.
pub fn ramp_share(t: f32, u: f32) -> f32 {
    let (min, max) = (keyed(&RAMP_MIN, t), keyed(&RAMP_MAX, t));
    let head = ((max - u) / RAMP_SOFTNESS + 1.).clamp(0., 1.);
    let tail = ((u - min) / RAMP_SOFTNESS + 1.).clamp(0., 1.);
    head * tail
}

/// The ring at `t` seconds into `load_circle_anim`: straight sRGB RGBA
/// texels, the ring's coverage (anti-aliased) times the ramp's alpha.
pub fn ring_texels(t: f32, rgba: &mut [u8]) {
    let n = RING_TEXELS as f32;
    let scale = n / RING_SIZE;
    let outer = RING_SIZE / 2. * scale;
    let inner = (RING_SIZE / 2. - RING_WIDTH) * scale;
    let grey = (RING_GREY * 255. + 0.5) as u8;
    for y in 0..RING_TEXELS {
        for x in 0..RING_TEXELS {
            let (dx, dy) = (x as f32 + 0.5 - n / 2., y as f32 + 0.5 - n / 2.);
            let d = dx.hypot(dy);
            let cover = (outer - d + 0.5).clamp(0., 1.) * (d - inner + 0.5).clamp(0., 1.);
            let a = if cover > 0. {
                // Clockwise from the top; y points down.
                let u = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.);
                RING_ALPHA * cover * ramp_share(t, u)
            } else {
                0.
            };
            let at = ((y * RING_TEXELS + x) * 4) as usize;
            if let Some(texel) = rgba.get_mut(at..at + 4) {
                texel.copy_from_slice(&[grey, grey, grey, (a * 255. + 0.5) as u8]);
            }
        }
    }
}

fn spawn_ring(ring: &mut ChildSpawnerCommands, image: Option<Handle<Image>>) {
    let centre = RING_SIZE / 2.;
    if let Some(image) = image {
        ring.spawn((
            RingImage(image.clone()),
            ImageNode::new(image),
            Node {
                position_type: PositionType::Absolute,
                width: px(RING_SIZE),
                height: px(RING_SIZE),
                ..default()
            },
        ));
    }
    let [r, g, b, a] = DISK_COLOUR;
    ring.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(centre - DISK_SIZE / 2.),
            top: px(centre - DISK_SIZE / 2.),
            width: px(DISK_SIZE),
            height: px(DISK_SIZE),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgba(r, g, b, a)),
    ));
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn present(
    fade: Res<Fade>,
    time: Res<Time<Real>>,
    borders: Option<Res<Borders>>,
    mut since: Local<Option<f32>>,
    mut overlay: Query<(&mut BackgroundColor, &mut Visibility), With<Overlay>>,
    mut loading: Query<&mut Visibility, (With<Loading>, Without<Overlay>)>,
    rings: Query<&RingImage>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut bars: Query<(&Bar, &mut Node)>,
) {
    if let Some(borders) = borders {
        let (top, bottom) = bar_shares(borders.frame());
        for (bar, mut node) in &mut bars {
            let height = percent(100. * if bar.top { top } else { bottom });
            if node.height != height {
                node.height = height;
            }
        }
    }
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
        // From empty when the loading text shows (`circle_rotate`).
        let start = *since.get_or_insert(time.elapsed_secs());
        let t = (time.elapsed_secs() - start) % CIRCLE_SECONDS;
        if let Some(images) = images.as_mut() {
            for ring in &rings {
                if let Some(mut image) = images.get_mut(&ring.0)
                    && let Some(data) = image.data.as_mut()
                {
                    ring_texels(t, data);
                }
            }
        }
    } else {
        *since = None;
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
pub const HANDED_ENV: &str = "GRID_HANDOFF_SHOW";
/// The ready marker of the screen that started this supervisor
/// ([`SelfHandoff`]): the supervisor's first process writes it.
#[cfg(not(target_os = "android"))]
pub const OUTER_READY_ENV: &str = "GRID_HANDOFF_OUTER_READY";

/// The frame on which a hidden window shows (after its first black frames).
pub(crate) const SHOW_FRAME: u32 = 3;

/// The window of a screen that installs the fade (`fades`): where the
/// previous screen's window was, and hidden until its first (black) frames
/// are drawn, so the desktop never shows the system's blank window. A
/// window without the fade opens as before.
pub fn window(mut window: Window, fades: bool) -> Window {
    if let Some(at) = window_at() {
        window.position = WindowPosition::At(at);
    } else if std::env::var_os(READY_ENV).is_some() {
        window.position = WindowPosition::Centered(MonitorSelection::Primary);
    }
    if std::env::var_os(HANDED_ENV).is_some() || (fades && cfg!(not(target_os = "android"))) {
        window.visible = false;
    }
    window
}

/// [`AT_ENV`]: where the previous screen's window was.
pub fn window_at() -> Option<IVec2> {
    std::env::var(AT_ENV).ok().and_then(|v| parse_at(&v))
}

/// The ready marker of the black screen that started this process
/// ([`OUTER_READY_ENV`]: a launcher race's [`SelfHandoff`]), for a process
/// that shows its window itself, the one-window front end (F767).
/// Written once: when the window shows, or, failing that, when it is
/// dropped (the process ends without showing), so the waiting screen never
/// waits the full limit.
#[cfg(not(target_os = "android"))]
pub struct OuterReady(Option<PathBuf>);

#[cfg(not(target_os = "android"))]
impl OuterReady {
    pub fn from_env() -> Self {
        Self(std::env::var_os(OUTER_READY_ENV).map(PathBuf::from))
    }

    /// A screen is still black until this process shows its window.
    pub fn waiting(&self) -> bool {
        self.0.is_some()
    }

    /// Writes the marker, if not written yet.
    pub fn write(&mut self) {
        if let Some(marker) = self.0.take()
            && let Err(error) = std::fs::write(&marker, b"ready
")
        {
            warn!("hand-off ready marker not written: {error}");
        }
    }
}

#[cfg(not(target_os = "android"))]
impl Drop for OuterReady {
    fn drop(&mut self) {
        self.write();
    }
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
#[allow(dead_code, reason = "for the career screen's supervisor (docs/SCREEN_FLOW.md §2.4)")]
pub fn run_supervised(
    first: std::process::Command,
    at: Option<String>,
    next: impl FnMut(i32) -> Option<std::process::Command>,
) -> std::io::Result<i32> {
    run_supervised_from(first, at, None, next)
}

/// [`run_supervised`] after a screen that started this supervisor itself
/// ([`SelfHandoff`]): `outer` is that screen's ready marker; the first
/// process starts hidden and writes it once it shows.
#[cfg(not(target_os = "android"))]
pub fn run_supervised_from(
    first: std::process::Command,
    at: Option<String>,
    outer: Option<PathBuf>,
    mut next: impl FnMut(i32) -> Option<std::process::Command>,
) -> std::io::Result<i32> {
    use std::process::{Child, Command};
    // A screen still black for the first process is let go when that
    // process ends without showing (a crash) and at the end.
    let release = || {
        if let Some(outer) = &outer
            && !outer.is_file()
        {
            let _ = std::fs::write(outer, b"ready\n");
        }
    };
    use std::time::Duration;
    let base = std::env::temp_dir().join(format!("grid-handoff-{}", std::process::id()));
    let go = base.with_extension("go");
    let ready = base.with_extension("ready");
    let clear = || {
        let _ = std::fs::remove_file(&go);
        let _ = std::fs::remove_file(&ready);
    };
    let start = |mut command: Command,
                 at: &Option<String>,
                 report: &std::path::Path,
                 handed: bool|
     -> std::io::Result<Child> {
        command
            .env(GO_ENV, &go)
            .env(READY_ENV, report)
            .env_remove(HANDED_ENV)
            .env_remove(OUTER_READY_ENV);
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
    // Where a process reports its first frame: the marker the black screen
    // before it waits for. The first process after a self hand-off reports
    // to that screen (`outer`); a hand-off passes the marker on, because the
    // leaving process waits for its own READY_ENV (F773: the menu's first Go
    // waited the full limit when the race reported elsewhere).
    let mut report = outer.clone().unwrap_or_else(|| ready.clone());
    let mut current = start(first, &at, &report, outer.is_some())?;
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
                release();
                clear();
                report = ready.clone();
                match next(code) {
                    Some(command) => current = start(command, &None, &report, false)?,
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
                let mut following = start(command, &at, &report, true)?;
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
                let _ = std::fs::remove_file(&report);
                current = following;
            }
        }
    };
    release();
    clear();
    Ok(result)
}

/// `CODE [X Y]` from a go marker: the code and the window position.
#[cfg(not(target_os = "android"))]
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

    /// Every `Last` system, bevy_winit's `changed_windows` among them, sees
    /// the window shown on the frame `handoff_ready` shows it (F773: in Last
    /// itself the order was ambiguous and a screen could stay hidden).
    #[test]
    fn the_shown_window_reaches_last_on_its_frame() {
        #[derive(Resource, Default)]
        struct Seen(Vec<bool>);
        let mut app = App::new();
        app.init_resource::<Seen>()
            .add_systems(Last, |window: Query<&Window>, mut seen: ResMut<Seen>| {
                seen.0.extend(window.iter().map(|w| w.visible));
            });
        add_window_show(&mut app);
        app.world_mut().spawn((Window { visible: false, ..default() }, PrimaryWindow));
        for _ in 0..SHOW_FRAME {
            app.update();
        }
        let seen = &app.world().resource::<Seen>().0;
        assert_eq!(seen.len(), SHOW_FRAME as usize);
        assert!(!seen[SHOW_FRAME as usize - 2]);
        assert!(seen[SHOW_FRAME as usize - 1]);
    }

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
    fn the_bars_cover_the_original_shares() {
        let (top, bottom) = bar_shares(0.);
        assert!((top - 0.13832).abs() < 1e-4 && (bottom - 0.14903).abs() < 1e-4, "{top} {bottom}");
        let (top, bottom) = bar_shares(0.3333);
        assert!((top - 0.10680).abs() < 1e-4 && (bottom - 0.11435).abs() < 1e-4, "{top} {bottom}");
        // Fully off at the default frame, and narrowing all the way.
        assert_eq!(bar_shares(BARS_OFF), (0., 0.));
        let mut last = bar_shares(0.);
        for i in 1..=20 {
            let now = bar_shares(BARS_OFF * i as f32 / 20.);
            assert!(now.0 <= last.0 && now.1 <= last.1);
            last = now;
        }
    }

    #[test]
    fn the_ring_fills_then_empties() {
        let full = |t: f32| (0..48).map(|i| ramp_share(t, (i as f32 + 0.5) / 48.)).sum::<f32>() / 48.;
        assert_eq!(full(0.), 0.);
        assert_eq!(full(1.6667), 1.);
        assert_eq!(full(CIRCLE_SECONDS), 0.);
        // Filling: the head runs clockwise from the top.
        assert!(ramp_share(0.6, 0.05) > ramp_share(0.6, 0.5));
        assert!(full(0.5) < full(1.0) && full(1.0) < full(1.6));
        // Emptying: the tail follows the same way.
        assert!(ramp_share(2.6, 0.1) < ramp_share(2.6, 0.9));
        assert!(full(2.0) > full(2.5) && full(2.5) > full(3.0));
    }

    #[test]
    fn the_ring_texture_follows_the_ramp() {
        let side = RING_TEXELS as usize;
        let alpha = |t: f32, x: usize, y: usize| {
            let mut rgba = vec![0; side * side * 4];
            ring_texels(t, &mut rgba);
            rgba[(y * side + x) * 4 + 3]
        };
        let top = (side / 2, 2);
        let right = (side - 3, side / 2);
        // Empty at the start, half alpha all round when full, never inside.
        assert_eq!(alpha(0., top.0, top.1), 0);
        assert_eq!(alpha(1.6667, top.0, top.1), 128);
        assert_eq!(alpha(1.6667, right.0, right.1), 128);
        assert_eq!(alpha(1.6667, side / 2, side / 2), 0);
        // Filling clockwise: the right side before the left.
        assert!(alpha(0.6, right.0, right.1) > alpha(0.6, 2, side / 2));
    }

    #[test]
    fn the_loading_text_waits_for_the_bars() {
        let mut borders = Borders::on();
        assert_eq!(bar_shares(borders.frame()), bar_shares(0.));
        borders.slide_off();
        borders.advance(0.05);
        assert!(!borders.off());
        for _ in 0..6 {
            borders.advance(0.05);
        }
        assert!(borders.off());
        assert_eq!(bar_shares(borders.frame()), (0., 0.));
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

    /// The leaving screen waits for its own READY_ENV, so the process it
    /// hands off to must report there, also after a self hand-off (`outer`).
    /// Stand-ins: `cmd` records the READY_ENV it got; the menu hands off 20.
    #[cfg(windows)]
    #[test]
    fn a_hand_off_passes_on_the_marker_the_leaving_screen_waits_for() {
        use std::os::windows::process::CommandExt;
        let dir = std::env::temp_dir().join(format!("grid-supervisor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = |name: &str, tail: &str| {
            let mut command = std::process::Command::new("cmd");
            command.raw_arg(format!("/C (echo %{READY_ENV}%)>\"{}\"{tail}", dir.join(name).display()));
            command
        };
        let outer = dir.join("outer.ready");
        let menu = script(
            "menu.env",
            &format!(" & (echo 20)>\"%{GO_ENV}%\" & ping -n 6 127.0.0.1 >nul"),
        );
        let mut race = Some(script("race.env", ""));
        let code = run_supervised_from(menu, None, Some(outer.clone()), |code| match code {
            20 => race.take(),
            _ => None,
        })
        .unwrap();
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap().trim().to_owned();
        assert_eq!(code, 0);
        assert_eq!(read("menu.env"), outer.display().to_string());
        assert_eq!(read("race.env"), outer.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
