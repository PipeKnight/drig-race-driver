//! Android window and display (F377, docs/ANDROID.md "Frame rate").
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
struct Display {
    refresh_hz: Option<f32>,
    /// Native window the hint was last sent to.
    hinted: Option<usize>,
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
    if let Some(hz) = display.refresh_hz
        && display.hinted != Some(ptr as usize)
    {
        display.hinted = Some(ptr as usize);
        // SAFETY: `native` keeps a reference to the window for this call.
        let result = unsafe { ANativeWindow_setFrameRate(ptr.cast(), hz, COMPATIBILITY_DEFAULT) };
        println!(
            "android_frame_rate requested_hz={hz} result={result} display_hz={:?}",
            crate::android::display_refresh_hz()
        );
    }
}

pub fn install(app: &mut App, refresh_hz: Option<f32>) {
    app.insert_resource(Display {
        refresh_hz,
        hinted: None,
    })
    .add_systems(PreUpdate, update);
}
