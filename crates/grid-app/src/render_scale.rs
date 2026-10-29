//! Render-height cap (`[video] render_height`, docs/SETTINGS.md): the 3D
//! view renders into an image of at most that many rows, and a second camera
//! stretches it over the window. Independently authored; GRID's PC renderer
//! always renders at the window size. On a phone, above Full HD costs GPU
//! time without a visible gain (owner, 2026-10-18); the mobile preset (Android
//! default, `mobile.rs`) renders 720 rows (owner, 2026-10-22: a PS3-era
//! picture on the phone is fine for a steady 60 FPS). The HUD, pause menu and touch zones are UI on the 3D camera, so
//! they are drawn into the same image and scaled with it (retargeting every
//! HUD module to the window camera was not a small change).
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderHeight {
    Native,
    /// A fixed row count.
    Rows(u32),
    /// Dynamic, driven by the frame time ([`Dynamic`], F556/F559): with the
    /// mobile preset between `mobile::DYNAMIC_FLOOR_ROWS` and
    /// `mobile::RENDER_ROWS`, on the desktop between half the window height
    /// and native (F573, [`auto_range`]).
    Auto,
}

impl RenderHeight {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("native") {
            return Ok(Self::Native);
        }
        if value.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        match value.parse::<u32>() {
            Ok(rows) if (240..=4320).contains(&rows) => Ok(Self::Rows(rows)),
            _ => Err("`render_height` needs auto, native or 240..4320".into()),
        }
    }

    /// The mobile preset (Android) and macOS: auto; other desktops: native.
    /// A Mac's built-in display is high-DPI, where native is 4K-class and
    /// `Auto` starts at [`HIDPI_CEILING_ROWS`] (owner, 2026-10-29, F642).
    pub fn platform_default() -> Self {
        if crate::mobile::enabled() || cfg!(target_os = "macos") {
            Self::Auto
        } else {
            Self::Native
        }
    }
}

/// `[video] present_mode`: only modes with a fallback chain, so an
/// unsupported choice cannot fail at swapchain set-up. Default `fifo` (Bevy's
/// default, strict vsync).
pub fn present_mode(value: &str) -> Result<bevy::window::PresentMode, String> {
    use bevy::window::PresentMode;
    match value.to_ascii_lowercase().as_str() {
        "fifo" => Ok(PresentMode::Fifo),
        "auto_vsync" => Ok(PresentMode::AutoVsync),
        "auto_no_vsync" => Ok(PresentMode::AutoNoVsync),
        _ => Err("`present_mode` needs fifo, auto_vsync or auto_no_vsync".into()),
    }
}

/// `[video] refresh_hz`: `system` (`None`, the desktop default) or 30..240 Hz,
/// a hint for the display rate (Android `ANativeWindow_setFrameRate`) and the
/// frame cap on Android (`settings::MOBILE_REFRESH_HZ` by default there).
pub fn refresh_hz(value: &str) -> Result<Option<f32>, String> {
    if value.eq_ignore_ascii_case("system") {
        return Ok(None);
    }
    match value.parse::<f32>() {
        Ok(hz) if (30. ..=240.).contains(&hz) => Ok(Some(hz)),
        _ => Err("`refresh_hz` needs system or 30..240".into()),
    }
}

/// Frame cap at the end of a frame: how long to sleep and when the next frame
/// is due, from the due time `due` (`None` on the first frame). Frames are
/// due every `period` on a fixed schedule, so a slightly late frame lets the
/// next one catch up and the average stays at the cap; a frame more than a
/// whole period late restarts the schedule. AUTHORED.
// Used by the Android display system and the tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn pace(
    now: std::time::Instant,
    due: Option<std::time::Instant>,
    period: std::time::Duration,
) -> (std::time::Duration, std::time::Instant) {
    let due = due.unwrap_or(now);
    let next = if now > due + period { now + period } else { due + period };
    (due.saturating_duration_since(now), next)
}

/// `[video] frame_latency`: 1..3 frames in flight (`None`: wgpu's default 2).
pub fn frame_latency(value: &str) -> Result<std::num::NonZero<u32>, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=3).contains(n))
        .and_then(std::num::NonZero::new)
        .ok_or_else(|| "`frame_latency` needs 1..3".into())
}

/// Render size for a window of `physical` pixels: `None` renders straight to
/// the window (native, or a cap at or above the window height, so never an
/// upscale beyond native). The width keeps the window's aspect ratio.
pub fn scaled_size(physical: UVec2, height: RenderHeight) -> Option<UVec2> {
    let RenderHeight::Rows(rows) = height else {
        return None;
    };
    if physical.x == 0 || physical.y == 0 || rows >= physical.y {
        return None;
    }
    let width = (u64::from(physical.x) * u64::from(rows) + u64::from(physical.y) / 2)
        / u64::from(physical.y);
    Some(UVec2::new(width.max(1) as u32, rows))
}

/// Dynamic render height (`render_height = auto`, F556/F559): between `floor`
/// and the ceiling `mobile::RENDER_ROWS`, driven by the frame time against
/// 60 FPS. Lowered at most every [`DOWN_SECONDS`] while frames run late and
/// the simulation is not the reason, by the rows a pixel-bound frame would need
/// to be on time, at least [`DOWN_ROWS`] and at most [`MAX_DOWN_SHARE`] of the
/// height (F642: 48-row steps were 2.5 % of the pixels at 1890 rows, lost in
/// the noise of a 68 ms smoke frame); raised by [`UP_ROWS`] after
/// [`UP_SECONDS`] of frames on time, never back to a height that ran late in
/// the last [`BLOCK_SECONDS`], so the sharpness does not "breathe".
///
/// Each step down is a probe (F571): [`PROBE_SECONDS`] later the frame must
/// have gained at least [`GPU_SHARE`] of what a frame bound only by the
/// pixel count would gain. Otherwise the rows were not what made it late
/// (the phone's race, F571: 41 -> 36 ms from 720 to 480 rows while the CPU
/// work holds it near 26 ms), so the height returns to the ceiling and
/// stays there for [`HOLD_SECONDS`] before the next probe. A frame that got
/// slower by more than the expected gain says the scene changed, not that the
/// pixels do not matter (F642: −46 ms against 2 ms as smoke thickened): the
/// probe is inconclusive and the lower height stays.
#[derive(Debug, Clone, PartialEq)]
pub struct Dynamic {
    pub floor: u32,
    /// The highest height ([`Self::set_range`]); 0 until the window is known.
    pub ceiling: u32,
    pub rows: u32,
    /// Smoothed frame time and fixed-step simulation time, ms.
    pub frame_ms: f32,
    /// Frame time smoothed over a few frames, for judging a probe.
    recent_ms: f32,
    pub work_ms: f32,
    warm: f32,
    since_change: f32,
    good_for: f32,
    /// Seconds late but simulation bound since the last `cpu_bound` report.
    cpu_held: f32,
    /// A height that ran late, and the seconds it stays blocked.
    blocked: Option<(u32, f32)>,
    /// The last step down under test: rows before and after, recent frame
    /// time before.
    probe: Option<(u32, u32, f32)>,
    /// Seconds left at the ceiling after a step that did not help.
    held: f32,
    /// Consecutive late frames (the smoothed time has settled).
    late_frames: u32,
    /// The last probe's gain and the gain of a pixel-bound frame, ms (log).
    pub gain: (f32, f32),
}

pub const TARGET_MS: f32 = 1000. / 60.;
/// Late: the smoothed frame time is above this (below about 55 FPS).
pub const LATE_MS: f32 = 18.;
/// On time: below this (60 FPS with vsync jitter).
pub const ON_TIME_MS: f32 = 17.2;
pub const DOWN_ROWS: u32 = 48;
/// The largest step down, as a share of the height (56 % of the pixels).
pub const MAX_DOWN_SHARE: f32 = 0.25;
pub const UP_ROWS: u32 = 24;
/// The desktop `Auto` ceiling on a high-DPI window (scale factor above 1):
/// 1440 rows (2K) upscaled instead of a 4K-class native height (owner,
/// 2026-10-29, F642: a Retina MacBook rendered 3024x1890).
pub const HIDPI_CEILING_ROWS: u32 = 1440;
pub const DOWN_SECONDS: f32 = 0.5;
pub const UP_SECONDS: f32 = 3.;
pub const BLOCK_SECONDS: f32 = 15.;
/// Simulation bound: the fixed-step loop (driving ticks) takes this share of
/// the frame, or more than [`CPU_MS`], a whole 60 FPS frame (a tyre pile):
/// the height holds. The phone's paused pre-race ticks took 10.7 ms (F549).
pub const CPU_SHARE: f32 = 0.5;
pub const CPU_MS: f32 = 16.;
const CPU_REPORT_SECONDS: f32 = 10.;
/// A step down is judged this long after it (the smoothing settles).
pub const PROBE_SECONDS: f32 = 1.;
/// A step must gain this share of a pixel-bound frame's gain to stay.
pub const GPU_SHARE: f32 = 0.5;
/// Seconds at the ceiling after a step that did not help.
pub const HOLD_SECONDS: f32 = 60.;
/// A step down waits for this many late frames, so that a sudden load is
/// probed against a settled frame time (88 % of the change).
pub const LATE_FRAMES: u32 = 20;
/// Frames longer than this (loading, a pause) are ignored. F642: dense smoke
/// on a MacBook held 87-102 ms frames for seconds, which 100 ms ignored.
const HITCH_MS: f32 = 250.;
const SMOOTHING: f32 = 0.1;
/// A probe second at 80 ms frames is 13 frames: the slow smoothing would
/// still be settling (F573).
const PROBE_SMOOTHING: f32 = 0.25;
const WARM_SECONDS: f32 = 3.;

impl Dynamic {
    pub fn new(floor: u32, ceiling: u32) -> Self {
        Self {
            floor: floor.min(ceiling),
            ceiling,
            rows: ceiling,
            frame_ms: TARGET_MS,
            recent_ms: TARGET_MS,
            work_ms: 0.,
            warm: 0.,
            since_change: 0.,
            good_for: 0.,
            cpu_held: 0.,
            blocked: None,
            probe: None,
            held: 0.,
            late_frames: 0,
            gain: (0., 0.),
        }
    }

    /// A new range (the desktop window changed size): start again at the
    /// ceiling. Returns whether it changed.
    pub fn set_range(&mut self, floor: u32, ceiling: u32) -> bool {
        if (floor.min(ceiling), ceiling) == (self.floor, self.ceiling) {
            return false;
        }
        *self = Self {
            warm: self.warm,
            frame_ms: self.frame_ms,
            recent_ms: self.recent_ms,
            ..Self::new(floor, ceiling)
        };
        true
    }

    /// The height for the next step down: where a pixel-bound frame would
    /// take [`TARGET_MS`], in whole [`UP_ROWS`], within the step limits.
    fn step_down(&self) -> u32 {
        let wanted = self.rows as f32 * (TARGET_MS / self.frame_ms).sqrt();
        let step = (self.rows as f32 - wanted).min(self.rows as f32 * MAX_DOWN_SHARE);
        let step = (step as u32 / UP_ROWS * UP_ROWS).max(DOWN_ROWS);
        self.rows.saturating_sub(step).max(self.floor)
    }

    /// One frame of `frame_ms` with `work_ms` of simulation; returns the
    /// reason when `rows` changed (`ineffective`: back to the ceiling after a
    /// probe), or `cpu_bound` every [`CPU_REPORT_SECONDS`] of late frames it
    /// held (for the log; `rows` unchanged).
    pub fn update(&mut self, frame_ms: f32, work_ms: f32, ceiling: u32) -> Option<&'static str> {
        if !(0. ..HITCH_MS).contains(&frame_ms) {
            return None;
        }
        let dt = frame_ms / 1000.;
        self.frame_ms += (frame_ms - self.frame_ms) * SMOOTHING;
        self.recent_ms += (frame_ms - self.recent_ms) * PROBE_SMOOTHING;
        self.work_ms += (work_ms - self.work_ms) * SMOOTHING;
        self.warm += dt;
        self.since_change += dt;
        if let Some((_, left)) = &mut self.blocked {
            *left -= dt;
        }
        self.blocked = self.blocked.filter(|(_, left)| *left > 0.);
        self.held = (self.held - dt).max(0.);
        if self.rows > ceiling {
            self.rows = ceiling;
            self.since_change = 0.;
            self.probe = None;
            return Some("ceiling");
        }
        if self.warm < WARM_SECONDS {
            return None;
        }
        if let Some((from, to, before)) = self.probe {
            if self.since_change < PROBE_SECONDS {
                return None;
            }
            self.probe = None;
            let pixels = (to as f32 / from as f32).powi(2);
            self.gain = (before - self.recent_ms, before * (1. - pixels));
            // Slower by more than the step could gain: inconclusive, it stays.
            let inconclusive = self.gain.0 < -self.gain.1;
            if !inconclusive && self.gain.0 < GPU_SHARE * self.gain.1 {
                self.rows = ceiling;
                self.blocked = None;
                self.held = HOLD_SECONDS;
                self.since_change = 0.;
                self.good_for = 0.;
                return Some("ineffective");
            }
        }
        let late = self.frame_ms > LATE_MS;
        self.late_frames = if late { self.late_frames + 1 } else { 0 };
        let cpu_bound = self.work_ms > CPU_SHARE * self.frame_ms || self.work_ms > CPU_MS;
        self.good_for = if self.frame_ms < ON_TIME_MS { self.good_for + dt } else { 0. };
        if self.late_frames >= LATE_FRAMES
            && !cpu_bound
            && self.held == 0.
            && self.since_change >= DOWN_SECONDS
            && self.rows > self.floor
        {
            let to = self.step_down();
            self.probe = Some((self.rows, to, self.recent_ms));
            self.blocked = Some((self.rows, BLOCK_SECONDS));
            self.rows = to;
            self.since_change = 0.;
            self.good_for = 0.;
            return Some("late");
        }
        self.cpu_held = if late && cpu_bound { self.cpu_held + dt } else { 0. };
        if self.cpu_held >= CPU_REPORT_SECONDS {
            self.cpu_held = 0.;
            return Some("cpu_bound");
        }
        let next = (self.rows + UP_ROWS).min(ceiling);
        if self.good_for >= UP_SECONDS
            && self.since_change >= UP_SECONDS
            && next > self.rows
            && !self.blocked.is_some_and(|(rows, _)| next >= rows)
        {
            self.rows = next;
            self.since_change = 0.;
            self.good_for = 0.;
            return Some("on_time");
        }
        None
    }
}

/// The setting and the sizes in use, read by the frame-rate counter.
#[derive(Resource, Debug)]
pub struct RenderScale {
    /// The setting.
    pub height: RenderHeight,
    /// Window size in physical pixels.
    pub screen: UVec2,
    /// The window's scale factor (above 1 on a high-DPI display).
    pub scale_factor: f32,
    /// Size the 3D view renders at (equal to `screen` when native).
    pub render: UVec2,
    /// The dynamic height, while the setting is `Auto`.
    pub dynamic: Option<Dynamic>,
    /// The Performance test holds the setting (no dynamic height).
    pub hold: bool,
    image: Option<Handle<Image>>,
    display: Option<Entity>,
}

impl RenderScale {
    /// A new cap from the Video screen, applied on the next frame.
    pub fn set_height(&mut self, height: RenderHeight) {
        if self.height != height {
            self.height = height;
            self.screen = UVec2::ZERO;
            self.dynamic = auto_dynamic(height);
        }
    }

    /// The height rendered now: under `Auto` the dynamic rows (the ceiling
    /// while held, with `GRID_DYNAMIC_RES=0` or before the desktop window is
    /// known: 720 rows with the mobile preset, else native).
    pub fn effective(&self) -> RenderHeight {
        match (&self.dynamic, self.height) {
            (Some(d), RenderHeight::Auto) if !self.hold && d.rows > 0 => RenderHeight::Rows(d.rows),
            (_, RenderHeight::Auto) if crate::mobile::enabled() => {
                RenderHeight::Rows(crate::mobile::RENDER_ROWS)
            }
            (_, RenderHeight::Auto) => RenderHeight::Native,
            _ => self.height,
        }
    }
}

/// The `Auto` range for a window `rows` high at `scale_factor`: with the
/// mobile preset 480-720 rows (owner, 2026-10-22); on the desktop native
/// down to half of it, rounded up to [`UP_ROWS`] (owner, 2026-10-26), with
/// the ceiling at most [`HIDPI_CEILING_ROWS`] on a high-DPI window (F642).
/// Native is the default on other desktops; `Auto` is chosen on the Video
/// screen.
pub fn auto_range(rows: u32, scale_factor: f32) -> (u32, u32) {
    if crate::mobile::enabled() {
        (crate::mobile::DYNAMIC_FLOOR_ROWS, crate::mobile::RENDER_ROWS)
    } else {
        let ceiling = if scale_factor > 1. { rows.min(HIDPI_CEILING_ROWS) } else { rows };
        ((ceiling / 2).div_ceil(UP_ROWS) * UP_ROWS, ceiling)
    }
}

/// The dynamic height for `Auto`, unless `GRID_DYNAMIC_RES=0`.
fn auto_dynamic(height: RenderHeight) -> Option<Dynamic> {
    let off = std::env::var("GRID_DYNAMIC_RES").is_ok_and(|v| v == "0");
    // The desktop range waits for the window size (`adjust_height`).
    let (floor, ceiling) = if crate::mobile::enabled() { auto_range(0, 1.) } else { (0, 0) };
    (height == RenderHeight::Auto && !off).then(|| Dynamic::new(floor, ceiling))
}

/// Window camera that shows the scaled image.
#[derive(Component)]
struct DisplayCamera;

#[derive(Component)]
struct DisplayImage;

#[allow(clippy::too_many_arguments)]
fn apply(
    mut commands: Commands,
    mut scale: ResMut<RenderScale>,
    window: Query<&Window, With<PrimaryWindow>>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut ImageNode, With<DisplayImage>>,
    touch: Option<Res<crate::touch_controls::Mobile>>,
    mut ui_scale: ResMut<UiScale>,
) {
    let (Some(w), Some(camera)) = (window.iter().next(), viewer.iter().next()) else {
        return;
    };
    let screen = UVec2::new(w.physical_width(), w.physical_height());
    if scale.scale_factor != w.scale_factor() {
        scale.scale_factor = w.scale_factor();
    }
    let wanted = scaled_size(screen, scale.effective());
    if screen == scale.screen && wanted.unwrap_or(screen) == scale.render {
        return;
    }
    scale.screen = screen;
    let Some(size) = wanted else {
        // Native: the 3D camera draws to the window as before.
        if scale.image.take().is_some() {
            commands.entity(camera).insert(RenderTarget::default());
            if let Some(display) = scale.display.take() {
                commands.entity(display).despawn();
            }
        }
        scale.render = screen;
        if touch.is_none() {
            ui_scale.0 = 1.;
        }
        println!(
            "render_scale height={:?} render={}x{} screen={}x{} mode=native",
            scale.effective(), screen.x, screen.y, screen.x, screen.y
        );
        return;
    };
    let image = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands
        .entity(camera)
        .insert(RenderTarget::Image(image.clone().into()));
    match scale.display {
        Some(_) => {
            for mut node in &mut shown {
                node.image = image.clone();
            }
        }
        None => {
            let display = commands
                .spawn((
                    DisplayCamera,
                    Camera2d,
                    Camera {
                        order: 1,
                        ..default()
                    },
                    Msaa::Off,
                    // The UI is drawn here, at the window's resolution (F556).
                    IsDefaultUiCamera,
                ))
                .id();
            commands.spawn((
                DisplayImage,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.),
                    height: percent(100.),
                    ..default()
                },
                ImageNode::new(image.clone()),
                GlobalZIndex(i32::MIN),
                UiTargetCamera(display),
            ));
            scale.display = Some(display);
        }
    }
    scale.image = Some(image);
    scale.render = size;
    // The UI is on the window camera, as when native.
    if touch.is_none() {
        ui_scale.0 = 1.;
    }
    println!(
        "render_scale height={:?} render={}x{} screen={}x{} mode=upscaled-image hud=window",
        scale.effective(), size.x, size.y, screen.x, screen.y
    );
}

/// Marks UI roots moved from the 3D camera to the window camera.
#[derive(Component)]
struct OnWindow;

/// With the scaled image shown, UI roots aimed at the 3D camera are drawn by
/// the window camera instead, so the HUD, menus and touch zones keep the
/// screen's full resolution (F556); back again when native.
fn ui_on_window(
    mut commands: Commands,
    scale: Res<RenderScale>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut roots: Query<(Entity, &mut UiTargetCamera, Has<OnWindow>), Without<DisplayImage>>,
) {
    let Some(viewer) = viewer.iter().next() else {
        return;
    };
    let (mut to_window, mut back) = (0, 0);
    for (entity, mut target, moved) in &mut roots {
        match scale.display {
            Some(display) if target.0 == viewer => {
                target.0 = display;
                commands.entity(entity).insert(OnWindow);
                to_window += 1;
            }
            None if moved => {
                target.0 = viewer;
                commands.entity(entity).remove::<OnWindow>();
                back += 1;
            }
            _ => {}
        }
    }
    if to_window + back > 0 {
        println!("render_scale ui_roots to_window={to_window} back_to_view={back}");
    }
}

/// Simulation time of this frame, ms: the fixed-step loop (the driving
/// ticks), which a lower resolution cannot speed up. F556 used the whole main
/// schedule, which on the phone is about as long as the frame (pipelined
/// rendering), so the height never dropped (F559).
#[derive(Resource, Default)]
struct Work {
    started: Option<std::time::Instant>,
    ms: f32,
}

fn work_start(mut work: ResMut<Work>) {
    work.started = Some(std::time::Instant::now());
}

fn work_end(mut work: ResMut<Work>) {
    if let Some(t) = work.started.take() {
        work.ms += t.elapsed().as_secs_f32() * 1000.;
    }
}

fn adjust_height(mut scale: ResMut<RenderScale>, mut work: ResMut<Work>, time: Res<Time<Real>>) {
    let work_ms = std::mem::take(&mut work.ms);
    if scale.height != RenderHeight::Auto || scale.hold || scale.screen.y == 0 {
        return;
    }
    let scale_factor = scale.scale_factor;
    let (floor, ceiling) = auto_range(scale.screen.y, scale_factor);
    let Some(d) = scale.dynamic.as_mut() else {
        return;
    };
    if d.set_range(floor, ceiling) {
        println!(
            "render_dynamic version=3 event=range floor={floor} ceiling={ceiling} scale_factor={scale_factor}"
        );
    }
    let from = d.rows;
    if let Some(reason) = d.update(time.delta_secs() * 1000., work_ms, ceiling) {
        println!(
            "render_dynamic version=3 rows={} from={from} reason={reason} frame_ms={:.1} work_ms={:.1} probe_gain_ms={:.2} pixel_gain_ms={:.2} floor={} ceiling={ceiling}",
            d.rows, d.frame_ms, d.work_ms, d.gain.0, d.gain.1, d.floor
        );
    }
}

/// Windowed runs only (offscreen captures keep their own image target).
pub fn install(app: &mut App, height: RenderHeight) {
    let dynamic = auto_dynamic(height);
    if let Some(d) = &dynamic {
        println!(
            "render_dynamic version=3 event=install floor={} ceiling={} target_ms={TARGET_MS:.2} late_ms={LATE_MS} on_time_ms={ON_TIME_MS} down_rows={DOWN_ROWS}..{MAX_DOWN_SHARE}/{DOWN_SECONDS}s hidpi_ceiling={HIDPI_CEILING_ROWS} hitch_ms={HITCH_MS} up_rows={UP_ROWS}/{UP_SECONDS}s block_s={BLOCK_SECONDS} cpu_share={CPU_SHARE} cpu_ms={CPU_MS} probe_s={PROBE_SECONDS} gpu_share={GPU_SHARE} hold_s={HOLD_SECONDS}",
            d.floor, d.rows
        );
    }
    app.insert_resource(RenderScale {
        height,
        screen: UVec2::ZERO,
        scale_factor: 1.,
        render: UVec2::ZERO,
        dynamic,
        hold: false,
        image: None,
        display: None,
    })
    .init_resource::<Work>()
    .add_systems(
        RunFixedMainLoop,
        (
            work_start.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            work_end.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        ),
    )
    .add_systems(Last, adjust_height)
    .add_systems(
        PostUpdate,
        (apply, ui_on_window)
            .chain()
            .before(bevy::ui::UiSystems::Prepare),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_native_and_rows() {
        assert_eq!(RenderHeight::parse("native"), Ok(RenderHeight::Native));
        assert_eq!(RenderHeight::parse("NATIVE"), Ok(RenderHeight::Native));
        assert_eq!(RenderHeight::parse("1080"), Ok(RenderHeight::Rows(1080)));
        assert_eq!(RenderHeight::parse("Auto"), Ok(RenderHeight::Auto));
        for bad in ["0", "100", "5000", "full", "-1080"] {
            assert!(RenderHeight::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn video_options_parse() {
        use bevy::window::PresentMode;
        assert_eq!(present_mode("FIFO"), Ok(PresentMode::Fifo));
        assert_eq!(present_mode("auto_no_vsync"), Ok(PresentMode::AutoNoVsync));
        assert!(present_mode("mailbox").is_err());
        assert_eq!(refresh_hz("system"), Ok(None));
        assert_eq!(refresh_hz("120"), Ok(Some(120.)));
        assert!(refresh_hz("10").is_err() && refresh_hz("fast").is_err());
        assert_eq!(frame_latency("1").map(|n| n.get()), Ok(1));
        assert!(frame_latency("0").is_err() && frame_latency("4").is_err());
    }

    #[test]
    fn pace_keeps_a_fixed_schedule() {
        use std::time::{Duration, Instant};
        let ms = Duration::from_millis;
        let t0 = Instant::now();
        let period = ms(16);
        // First frame: no sleep, the next is due a period later.
        assert_eq!(pace(t0, None, period), (ms(0), t0 + ms(16)));
        // Early: sleep to the due time.
        assert_eq!(pace(t0 + ms(10), Some(t0 + ms(16)), period), (ms(6), t0 + ms(32)));
        // A little late: no sleep, and the next stays on the schedule.
        assert_eq!(pace(t0 + ms(36), Some(t0 + ms(32)), period), (ms(0), t0 + ms(48)));
        // More than a period late: the schedule restarts from now.
        assert_eq!(pace(t0 + ms(70), Some(t0 + ms(48)), period), (ms(0), t0 + ms(86)));
    }

    /// Feeds `seconds` of frames of `frame_ms` with `work_ms` of main work.
    fn run(d: &mut Dynamic, seconds: f32, frame_ms: f32, work_ms: f32) -> Vec<u32> {
        run_with(d, seconds, |_| frame_ms, work_ms)
    }

    /// Frames whose time depends on the rows rendered.
    fn run_with(d: &mut Dynamic, seconds: f32, frame: impl Fn(u32) -> f32, work_ms: f32) -> Vec<u32> {
        run_with_ceiling(d, seconds, frame, work_ms, 720)
    }

    fn run_with_ceiling(
        d: &mut Dynamic,
        seconds: f32,
        frame: impl Fn(u32) -> f32,
        work_ms: f32,
        ceiling: u32,
    ) -> Vec<u32> {
        let mut seen = Vec::new();
        let mut t = 0.;
        while t < seconds * 1000. {
            let ms = frame(d.rows);
            t += ms;
            let from = d.rows;
            if d.update(ms, work_ms, ceiling).is_some() && d.rows != from {
                seen.push(d.rows);
            }
        }
        seen
    }

    /// A frame bound by the pixel count: `ms` at 720 rows.
    fn pixel_bound(ms: f32) -> impl Fn(u32) -> f32 {
        move |rows| ms * (rows as f32 / 720.).powi(2)
    }

    #[test]
    fn dynamic_height_drops_fast_when_late_and_rises_slowly() {
        let mut d = Dynamic::new(480, 720);
        // Warm-up: no change in the first seconds.
        assert!(run_with(&mut d, 2.5, pixel_bound(45.), 5.).is_empty());
        // Frames bound by the pixel count (45 ms at 720 rows): a quarter of
        // the height in whole 24-row units (168 rows), then to the floor;
        // each step gains what it should.
        let down = run_with(&mut d, 6., pixel_bound(45.), 5.);
        assert_eq!(down, vec![552, 480]);
        // On time: up 24 rows every 3 s, but not back to 552 (late 1 s ago)
        // until its block ends.
        assert_eq!(run(&mut d, 10., 16.7, 5.), vec![504, 528]);
        // Then up 24 rows every 3 s to the ceiling.
        let later = run(&mut d, 60., 16.7, 5.);
        assert!(later.windows(2).all(|w| w[1] == w[0] + 24), "{later:?}");
        assert_eq!(later.last(), Some(&720));
    }

    #[test]
    fn dynamic_height_ignores_a_main_thread_bound_frame() {
        let mut d = Dynamic::new(480, 720);
        // A tyre pile: 40 ms frames, 35 ms of them simulation.
        assert!(run(&mut d, 10., 40., 35.).is_empty());
        assert_eq!(d.rows, 720);
        // The hold is reported every 10 s of late frames (log only).
        let reports = (0..250).filter(|_| d.update(40., 35., 720) == Some("cpu_bound")).count();
        assert_eq!(reports, 1);
        assert_eq!(d.rows, 720);
        // Late with a light simulation and pixel-bound frames: the height
        // drops.
        assert_eq!(run_with(&mut d, 2., pixel_bound(30.), 4.), vec![552]);
        // Loading hitches are ignored too.
        assert!(run(&mut d, 10., 500., 1.).is_empty());
        // A lower ceiling (the Video screen) applies at once.
        assert_eq!(d.update(16.7, 1., 504), Some("ceiling"));
        assert_eq!(d.rows, 504);
    }

    /// F571: the phone's race (36-41 ms frames, a light fixed step) went
    /// 720 -> 480 rows and stayed there, although the rows hardly changed the
    /// frame. A step that does not help returns to the ceiling and holds.
    #[test]
    fn dynamic_height_returns_when_a_step_does_not_help() {
        let mut d = Dynamic::new(480, 720);
        assert!(run(&mut d, 2.9, 37., 1.5).is_empty());
        // CPU-bound: 37 ms whatever the rows. One probe, then back.
        assert_eq!(run(&mut d, 2., 37., 1.5), vec![552, 720]);
        assert!(d.gain.0.abs() < 0.5 && d.gain.1 > 4.5, "{:?}", d.gain);
        // Held at the ceiling for HOLD_SECONDS, then one more probe.
        assert!(run(&mut d, 58., 37., 1.5).is_empty());
        assert_eq!(run(&mut d, 4., 37., 1.5), vec![552, 720]);
        // Mostly CPU-bound (a quarter of the frame scales with the pixels):
        // still back.
        let mostly_cpu = |rows: u32| 28. + 9. * (rows as f32 / 720.).powi(2);
        assert!(run_with(&mut d, 57., mostly_cpu, 1.5).is_empty());
        assert_eq!(run_with(&mut d, 4., mostly_cpu, 1.5), vec![552, 720]);
        assert_eq!(d.rows, 720);
    }

    /// F573: desktop Auto runs from native down to half of it; at full speed
    /// it stays native.
    #[test]
    fn desktop_auto_range_starts_native_and_stays_there_on_time() {
        let (floor, ceiling) = (684u32.div_ceil(UP_ROWS) * UP_ROWS, 1369);
        assert_eq!((floor, ceiling), (696, 1369));
        let mut d = Dynamic::new(0, 0);
        assert!(d.set_range(floor, ceiling));
        assert!(!d.set_range(floor, ceiling));
        assert_eq!(d.rows, 1369);
        // 7 ms frames for a minute: no change, and native means no image.
        assert!(run_with_ceiling(&mut d, 60., |_| 7., 1., 1369).is_empty());
        assert_eq!(d.rows, 1369);
        assert_eq!(scaled_size(UVec2::new(2560, 1369), RenderHeight::Rows(d.rows)), None);
        // Pixel-bound late frames step down to the floor and not below.
        let frame = |rows: u32| 80. * (rows as f32 / 1369.).powi(2);
        let down = run_with_ceiling(&mut d, 30., frame, 1., 1369);
        assert_eq!(down.last(), Some(&floor), "{down:?}");
        // A resized window starts again at its native height.
        assert!(d.set_range(540, 1080));
        assert_eq!(d.rows, 1080);
    }

    /// F642: a Retina MacBook (3024 × 1890, scale factor 2) rendered 1890
    /// rows under Auto; a high-DPI window now tops out at 1440.
    #[test]
    fn dynamic_height_hidpi_range() {
        assert_eq!(auto_range(1890, 2.), (720, 1440));
        assert_eq!(auto_range(1890, 1.), (960, 1890));
        assert_eq!(auto_range(1080, 1.5), (552, 1080));
        assert_eq!(
            scaled_size(UVec2::new(3024, 1890), RenderHeight::Rows(1440)),
            Some(UVec2::new(2304, 1440))
        );
    }

    /// F642: 48-row steps were 2.5 % of the pixels at 1890 rows. A step now
    /// aims at an on-time pixel-bound frame, at most a quarter of the height.
    #[test]
    fn dynamic_height_steps_by_the_frame_time() {
        let mut d = Dynamic::new(0, 0);
        d.set_range(720, 1440);
        assert!(run_with_ceiling(&mut d, 2.9, |_| 7., 1., 1440).is_empty());
        // Dense smoke: 68 ms at 1440 rows, bound by the pixels.
        let frame = |rows: u32| 68. * (rows as f32 / 1440.).powi(2);
        let down = run_with_ceiling(&mut d, 5., frame, 2., 1440);
        assert_eq!(down, vec![1080, 816, 720]);
        // Slightly late at the ceiling: still the smallest step.
        let mut d = Dynamic::new(480, 720);
        let down = run_with(&mut d, 6., pixel_bound(19.), 1.);
        assert_eq!(down.first(), Some(&672), "{down:?}");
        // Frames of 100-250 ms count; longer ones are hitches.
        let mut d = Dynamic::new(480, 720);
        assert_eq!(run(&mut d, 8., 120., 1.).first(), Some(&552));
        let mut d = Dynamic::new(480, 720);
        assert!(run(&mut d, 10., 300., 1.).is_empty());
    }

    /// F642: as smoke thickened, a probe measured −46 ms against 2 ms
    /// expected and went back to native. A frame slower by more than the
    /// expected gain is inconclusive: the step stays.
    #[test]
    fn dynamic_height_keeps_a_step_when_the_scene_gets_slower() {
        let mut d = Dynamic::new(480, 720);
        assert!(run(&mut d, 2.9, 30., 1.).is_empty());
        let mut reasons = Vec::new();
        let mut t = 0.;
        while t < 3000. {
            // 30 ms frames until the first step, then a heavier scene: 60 ms
            // at 540 rows, still bound by the pixels.
            let ms = if d.rows == 720 { 30. } else { 60. * (d.rows as f32 / 540.).powi(2) };
            t += ms;
            if let Some(reason) = d.update(ms, 1., 720) {
                reasons.push((reason, d.rows));
            }
        }
        assert_eq!(reasons.first(), Some(&("late", 552)), "{reasons:?}");
        assert!(reasons.iter().all(|(r, _)| *r != "ineffective"), "{reasons:?}");
        // The next step, to the floor, gained what the pixels promised.
        assert_eq!(d.rows, 480, "{reasons:?}");
        assert!(d.gain.0 > GPU_SHARE * d.gain.1, "{:?}", d.gain);
    }

    #[test]
    fn caps_height_keeps_aspect_and_never_upscales() {
        let rows = RenderHeight::Rows(1080);
        // Xiaomi 13 Ultra WQHD+ landscape.
        assert_eq!(
            scaled_size(UVec2::new(3200, 1440), rows),
            Some(UVec2::new(2400, 1080))
        );
        // Already at or below the cap: straight to the window.
        assert_eq!(scaled_size(UVec2::new(2400, 1080), rows), None);
        assert_eq!(scaled_size(UVec2::new(1280, 800), rows), None);
        assert_eq!(
            scaled_size(UVec2::new(3200, 1440), RenderHeight::Native),
            None
        );
        assert_eq!(scaled_size(UVec2::ZERO, rows), None);
        // Odd ratios round to the nearest column.
        assert_eq!(
            scaled_size(UVec2::new(3088, 1440), RenderHeight::Rows(720)),
            Some(UVec2::new(1544, 720))
        );
        assert_eq!(
            scaled_size(UVec2::new(2001, 1000), RenderHeight::Rows(500)),
            Some(UVec2::new(1001, 500))
        );
    }
}
