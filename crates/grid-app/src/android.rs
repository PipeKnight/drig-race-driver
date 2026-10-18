//! Android start-up (P9, docs/PLATFORMS_AND_INPUT_RESEARCH.md 7.5).
//!
//! The app has no argv, shell or working directory on Android. This module
//! sends stdout and stderr to logcat (tag `grid`) and to a log file in the
//! shared Downloads folder (Download/GRID-Reassembly, opened by
//! `MainActivity.openLogFile`), shows start-up errors and panics on screen
//! (`MainActivity.showFatal`), and provides the app-specific external storage
//! folder that holds the game data (docs/ANDROID.md).

use std::ffi::{CString, c_char, c_int};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use jni::JNIEnv;
use jni::objects::{JObject, JValue};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

const ANDROID_LOG_INFO: c_int = 4;
const ANDROID_LOG_ERROR: c_int = 6;

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static STARTED: OnceLock<Instant> = OnceLock::new();
static FATAL_SHOWN: AtomicBool = AtomicBool::new(false);

/// Writes one line to logcat and the log file directly (bypasses the stdout
/// pipe, so it is not lost when the process exits right after).
pub fn log(priority_error: bool, text: &str) {
    let priority = if priority_error { ANDROID_LOG_ERROR } else { ANDROID_LOG_INFO };
    let line = CString::new(text.replace('\0', " ")).unwrap_or_default();
    // SAFETY: both pointers are valid NUL-terminated strings for the call.
    unsafe {
        __android_log_write(priority, c"grid".as_ptr(), line.as_ptr());
    }
    if let Some(file) = LOG_FILE.get() {
        let seconds = STARTED.get().map_or(0.0, |t| t.elapsed().as_secs_f64());
        let mark = if priority_error { "E" } else { " " };
        if let Ok(mut file) = file.lock() {
            // Unbuffered: every line reaches the file even if the process aborts.
            let _ = writeln!(file, "[{seconds:9.3}] {mark} {text}");
        }
    }
}

/// Opens the log file, redirects stdout and stderr into a pipe read line by
/// line into [`log`], and installs the panic hook.
pub fn init() {
    STARTED.get_or_init(Instant::now);
    match with_activity(|env, activity| env.call_method(activity, "openLogFile", "()I", &[])?.i()) {
        // SAFETY: Java detached the descriptor and handed its ownership over.
        Some(fd) if fd >= 0 => drop(LOG_FILE.set(Mutex::new(unsafe { File::from_raw_fd(fd) }))),
        _ => log(true, "log file: Download/GRID-Reassembly could not be opened; logcat only"),
    }
    log(
        false,
        &format!(
            "GRID Reassembly {} (android, revision {})",
            env!("CARGO_PKG_VERSION"),
            super::BUILD_REVISION
        ),
    );
    install_panic_hook();

    let mut fds = [0 as c_int; 2];
    // SAFETY: `fds` has room for the two descriptors pipe() writes.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        log(true, "stdout redirect: pipe failed");
        return;
    }
    let [read, write] = fds;
    // SAFETY: plain descriptor duplication; 1 and 2 are the process's stdout/stderr.
    unsafe {
        libc::dup2(write, 1);
        libc::dup2(write, 2);
        libc::close(write);
    }
    std::thread::Builder::new()
        .name("grid-logcat".into())
        .spawn(move || {
            // SAFETY: `read` is the pipe's read end, owned only by this thread.
            let file = unsafe { File::from_raw_fd(read) };
            for line in BufReader::new(file).split(b'\n').map_while(Result::ok) {
                log(false, &String::from_utf8_lossy(&line));
            }
        })
        .map(drop)
        .unwrap_or_else(|_| log(true, "stdout redirect: thread failed"));
}

/// Logs a panic directly (the default hook's stderr text goes through the
/// pipe and may be lost on abort) and shows it on screen; then runs the
/// previous hook.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        fatal(&format!("panic in thread '{}': {info}", thread.name().unwrap_or("unnamed")));
        previous(info);
    }));
}

/// Logs `message` as an error and shows it on screen (ErrorActivity, a
/// separate process that stays when this one exits). Only the first fatal
/// message is shown; later ones are only logged.
pub fn fatal(message: &str) {
    log(true, message);
    if FATAL_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }
    let shown = with_activity(|env, activity| {
        let text = env.new_string(message)?;
        env.call_method(activity, "showFatal", "(Ljava/lang/String;)V", &[JValue::Object(&text)])?;
        Ok(())
    });
    if shown.is_none() {
        log(true, "error screen: MainActivity.showFatal failed");
    }
}

/// Calls into the Java `MainActivity` from any thread (attached on demand).
/// A Java exception is logged, cleared and turned into `None`.
fn with_activity<R>(
    call: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>,
) -> Option<R> {
    let app = bevy::android::ANDROID_APP.get()?;
    // SAFETY: android-activity owns a valid JavaVM pointer for the process.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    // SAFETY: a global reference to the activity, valid until onDestroy; the
    // JObject does not delete it when dropped.
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let result = call(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result.ok()
}

/// `Android/data/<package>/files`: readable and writable without any
/// permission, reachable by `adb push`, deleted with the app. The bundled APK
/// copies its game data here on the first start (LauncherActivity.java).
pub fn data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP
        .get()
        .and_then(|app| app.external_data_path())
}

/// Whether the device has a finger touch screen (AConfiguration
/// `ACONFIGURATION_TOUCHSCREEN_FINGER` = 3): `[input] touch=auto` turns the
/// phone controls on then (P11).
pub fn touch_screen() -> bool {
    bevy::android::ANDROID_APP
        .get()
        .is_some_and(|app| i32::from(app.config().touchscreen()) == 3)
}

/// The display's current refresh rate (`Activity.getDisplay().getRefreshRate()`,
/// API 30); winit reports none on Android.
pub fn display_refresh_hz() -> Option<f32> {
    with_activity(|env, activity| {
        let display = env
            .call_method(activity, "getDisplay", "()Landroid/view/Display;", &[])?
            .l()?;
        env.call_method(&display, "getRefreshRate", "()F", &[])?.f()
    })
}
