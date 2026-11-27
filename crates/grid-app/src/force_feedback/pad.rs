//! Shared effect task for Android and macOS. The worker evaluates the same pad effects
//! as Windows every 10 ms. Android submits them to its vibrator API, while macOS reads
//! the latest motor levels on the thread that owns the SDL gamepad.

#[cfg(target_os = "android")]
use super::Vibration;
use super::{Ffb, Output, motors};
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
    #[cfg(target_os = "macos")]
    pub levels: (u16, u16),
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
            #[cfg(target_os = "android")]
            crate::android::log(true, &format!("ffb: no thread: {error}"));
            #[cfg(target_os = "macos")]
            eprintln!("ffb: no thread: {error}");
        }
        Handle { shared }
    })
}

fn run(shared: &Arc<Mutex<Shared>>) {
    #[cfg(target_os = "android")]
    crate::android::attach_thread();
    #[cfg(target_os = "android")]
    let mut vibration = Vibration::default();
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let now = Instant::now();
        let dt = (now - last).as_secs_f64();
        last = now;
        #[cfg(target_os = "android")]
        let levels;
        {
            let Ok(mut s) = shared.lock() else { return };
            let fed = s.fed.is_some_and(|t| now - t < Duration::from_millis(250));
            let out = match &s.ffb {
                Some(ffb) if fed && s.rumble => ffb.output(),
                _ => Output::default(),
            };
            if let Some(ffb) = s.ffb.as_mut() {
                ffb.process(dt);
            }
            let levels_now = motors(&out);
            #[cfg(target_os = "macos")]
            {
                s.levels = levels_now;
            }
            #[cfg(target_os = "android")]
            {
                levels = levels_now;
            }
        }
        #[cfg(target_os = "android")]
        if let Some((left, right)) = vibration.due(now, levels) {
            crate::android::rumble(left, right);
        }
    }
}
