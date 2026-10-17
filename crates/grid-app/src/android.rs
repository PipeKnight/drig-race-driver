//! Android start-up (P9, docs/PLATFORMS_AND_INPUT_RESEARCH.md 7.5).
//!
//! The app has no argv, shell or working directory on Android. This module
//! sends stdout and stderr to logcat (tag `grid`), and provides the
//! app-specific external storage folder, where `adb push` places the game data
//! (docs/ANDROID.md).

use std::ffi::{CString, c_char, c_int};
use std::io::{BufRead, BufReader};
use std::os::fd::FromRawFd;
use std::path::PathBuf;

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

const ANDROID_LOG_INFO: c_int = 4;
const ANDROID_LOG_ERROR: c_int = 6;

/// Writes one line to logcat directly (bypasses the stdout pipe, so it is not
/// lost when the process exits right after).
pub fn log(priority_error: bool, text: &str) {
    let priority = if priority_error { ANDROID_LOG_ERROR } else { ANDROID_LOG_INFO };
    let text = CString::new(text.replace('\0', " ")).unwrap_or_default();
    // SAFETY: both pointers are valid NUL-terminated strings for the call.
    unsafe {
        __android_log_write(priority, c"grid".as_ptr(), text.as_ptr());
    }
}

/// Redirects stdout and stderr into a pipe read line by line into logcat.
pub fn init() {
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
            let file = unsafe { std::fs::File::from_raw_fd(read) };
            for line in BufReader::new(file).split(b'\n').map_while(Result::ok) {
                log(false, &String::from_utf8_lossy(&line));
            }
        })
        .map(drop)
        .unwrap_or_else(|_| log(true, "stdout redirect: thread failed"));
}

/// `Android/data/<package>/files`: readable and writable without any
/// permission, reachable by `adb push`, deleted with the app.
pub fn data_dir() -> Option<PathBuf> {
    bevy::android::ANDROID_APP
        .get()
        .and_then(|app| app.external_data_path())
}
