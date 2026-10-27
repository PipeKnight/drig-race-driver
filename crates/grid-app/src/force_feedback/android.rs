//! Android rumble [A] (docs/FORCE_FEEDBACK.md §7.2): the original's `XBOX360 Controller` device,
//! whose motors carry only the effect samples (surfaces, kerbs, gear changes, collisions, skids,
//! wheel damage), played on a connected pad's vibrators, else on the phone's own vibrator.
//!
//! The task runs on its own thread as on Windows (`Sleep(10)`, then `7ECAC0`); the levels go to
//! `MainActivity.rumble` through `Vibration`'s rate limit.

use super::{Ffb, Output, Vibration, motors};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// State shared between the game and the FFB thread.
#[derive(Default)]
pub struct Shared {
    /// The FFB task, set by the race (`crate::ffb`).
    pub ffb: Option<Ffb>,
    /// Send the effect channels to the vibrators.
    pub rumble: bool,
    /// When the race last queued a snapshot; the outputs go to zero 0.25 s after [A].
    pub fed: Option<Instant>,
}

pub struct Handle {
    pub shared: Arc<Mutex<Shared>>,
}

static DEVICE: OnceLock<Handle> = OnceLock::new();

/// The process's FFB thread, started on first use.
pub fn device() -> &'static Handle {
    DEVICE.get_or_init(|| {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let thread_shared = shared.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("FFB updateTask".into())
            .spawn(move || run(&thread_shared))
        {
            crate::android::log(true, &format!("ffb: no thread: {error}"));
        }
        Handle { shared }
    })
}

fn run(shared: &Arc<Mutex<Shared>>) {
    crate::android::attach_thread();
    let mut vibration = Vibration::default();
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let now = Instant::now();
        let dt = (now - last).as_secs_f64();
        last = now;
        let levels = {
            let Ok(mut s) = shared.lock() else { return };
            let fed = s.fed.is_some_and(|t| now - t < Duration::from_millis(250));
            let out = match &s.ffb {
                Some(ffb) if fed && s.rumble => ffb.output(),
                _ => Output::default(),
            };
            if let Some(ffb) = s.ffb.as_mut() {
                ffb.process(dt);
            }
            motors(&out)
        };
        if let Some((left, right)) = vibration.due(now, levels) {
            crate::android::rumble(left, right);
        }
    }
}
