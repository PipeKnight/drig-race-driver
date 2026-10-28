//! Android window, display and frame cap (F378, docs/ANDROID.md "Frame rate").
//!
//! Bevy's `Window` starts with the requested 1280 x 800 and scale factor 1;
//! on Android winit ignores the request, and the first phone log
//! (`frame_setup physical=1280x800 scale=1`) showed the resolution can stay
//! there, so the camera aspect and the render cap would follow a size the
//! screen does not have. This system makes the window follow the native
//! surface (`ANativeWindow` width/height) and the display density, and can
//! send the `[video] refresh_hz` hint through `ANativeWindow_setFrameRate`.
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[link(name = "nativewindow")]
unsafe extern "C" {
    /// API 30 (`android/native_window.h`).
    fn ANativeWindow_setFrameRate(
        window: *mut std::ffi::c_void,
        frame_rate: f32,
        compatibility: i8,
    ) -> i32;
}

/// `ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_DEFAULT`.
const COMPATIBILITY_DEFAULT: i8 = 0;

#[derive(Resource)]
pub struct Display {
    refresh_hz: Option<f32>,
    /// Native window the hint was last sent to.
    hinted: Option<usize>,
    /// A hint was sent, so `None` must be sent as 0 (no preference).
    sent: bool,
}

impl Display {
    /// A new hint from the Video screen, sent on the next frame.
    pub fn set_refresh(&mut self, hz: Option<f32>) {
        if self.refresh_hz != hz {
            self.refresh_hz = hz;
            self.hinted = None;
        }
    }
}

fn update(mut display: ResMut<Display>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Some(app) = bevy::android::ANDROID_APP.get() else {
        return;
    };
    let Some(native) = app.native_window() else {
        return;
    };
    let (w, h) = (native.width(), native.height());
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let scale = app.config().density().map_or(1., |dpi| dpi as f32 / 160.);
    if w > 0
        && h > 0
        && (window.physical_width() != w as u32
            || window.physical_height() != h as u32
            || (window.scale_factor() - scale).abs() > 1e-3)
    {
        println!(
            "android_surface native={w}x{h} density_scale={scale} window_before={}x{} scale_before={}",
            window.physical_width(),
            window.physical_height(),
            window.scale_factor()
        );
        window.resolution.set_scale_factor(scale);
        window
            .resolution
            .set_physical_resolution(w as u32, h as u32);
    }
    let ptr = native.ptr().as_ptr();
    if (display.refresh_hz.is_some() || display.sent) && display.hinted != Some(ptr as usize) {
        // 0 Hz clears an earlier hint (no preference).
        let hz = display.refresh_hz.unwrap_or(0.);
        display.hinted = Some(ptr as usize);
        display.sent = display.refresh_hz.is_some();
        // SAFETY: `native` keeps a reference to the window for this call.
        let result = unsafe { ANativeWindow_setFrameRate(ptr.cast(), hz, COMPATIBILITY_DEFAULT) };
        println!(
            "android_frame_rate requested_hz={hz} result={result} display_hz={:?}",
            crate::android::display_refresh_hz()
        );
    }
}

/// Frame cap at `refresh_hz` (60 Hz by default): the display may run faster
/// than the hint asks (a 120 Hz panel), and Fifo alone would then present at
/// the panel's rate. Sleeps at the end of the main schedule; System has no cap.
fn cap(display: Res<Display>, mut due: Local<Option<std::time::Instant>>) {
    let Some(hz) = display.refresh_hz else {
        *due = None;
        return;
    };
    let period = std::time::Duration::from_secs_f32(1. / hz);
    let (sleep, next) = crate::render_scale::pace(std::time::Instant::now(), *due, period);
    if !sleep.is_zero() {
        std::thread::sleep(sleep);
    }
    *due = Some(next);
}

pub fn install(app: &mut App, refresh_hz: Option<f32>) {
    println!("android_frame_cap hz={refresh_hz:?}");
    app.insert_resource(Display {
        refresh_hz,
        hinted: None,
        sent: false,
    })
    .add_systems(PreUpdate, update)
    .add_systems(Last, cap);
}
