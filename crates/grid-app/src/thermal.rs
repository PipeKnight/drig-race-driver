//! Phone thermal headroom and an optional thermal governor (F935,
//! docs/ANDROID_CPU.md section 4).
//!
//! A 15-minute phone soak (lean, 60 cap) throttles after about two minutes:
//! FPS falls from 32 to about 25 and stays there. The sustained budget is the
//! hot phone, so work per second matters as much as the frame time.
//!
//! Always on under Android: a `grid-thermal` thread acquires the NDK
//! `AThermalManager` (API 30, looked up with `dlsym`, so older systems only
//! log the miss), registers a thermal status listener, and once a second
//! reads `AThermal_getThermalHeadroom` with a [`FORECAST_S`] forecast (API
//! 31; more than one call per second is not allowed). 1.0 is the headroom at
//! which the system reaches the SEVERE status. Where the system has them (API
//! 35), the start log lists the headroom thresholds of each status. Every
//! frame_stats line gains `thermal_h` (the last forecast headroom),
//! `thermal_status` and `thermal_level`.
//!
//! The governor is off by default. `GRID_THERMAL_GOV` lists its steps in
//! order (comma separated); each level adds the next step:
//!
//! - `scale`: the dynamic render height's ceiling drops to [`SCALE_SHARE`] of
//!   itself (`RenderScale::thermal_ceiling`; GPU work per frame);
//! - `cap`: the frame cap drops to [`CAP_HZ`] (`android_display`; all work
//!   per second, CPU and GPU).
//!
//! A sample is hot when the headroom reaches `GRID_THERMAL_HOT` (default
//! [`HOT`]) or the status reaches MODERATE; it is cool when the headroom is at
//! most `GRID_THERMAL_COOL` (default [`COOL`]) and the status is below
//! MODERATE. Hysteresis: a hot sample raises the level at most once per
//! [`UP_DWELL_S`]; the level falls one step after [`COOL_HOLD_S`] of
//! uninterrupted cool samples. Nothing here depends on a track, a car or a
//! phone model. Other systems compile the platform part to no-ops; the
//! governor logic is plain and tested everywhere.
use bevy::prelude::*;

/// The forecast `AThermal_getThermalHeadroom` is asked for, in seconds.
pub const FORECAST_S: i32 = 10;
/// Default hot headroom (`GRID_THERMAL_HOT`).
pub const HOT: f32 = 0.8;
/// Default cool headroom (`GRID_THERMAL_COOL`).
pub const COOL: f32 = 0.6;
/// `ATHERMAL_STATUS_MODERATE`: a status at least this high is hot.
pub const HOT_STATUS: i32 = 2;
/// Shortest time between two raises of the level.
pub const UP_DWELL_S: f32 = 5.;
/// Cool time before the level falls one step (and the shortest time between
/// any change and a fall).
pub const COOL_HOLD_S: f32 = 30.;
/// The `scale` step's render-height ceiling, as a share of the ceiling.
pub const SCALE_SHARE: f32 = 0.8;
/// The `cap` step's frame cap.
pub const CAP_HZ: f32 = 30.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Scale,
    Cap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub steps: Vec<Step>,
    pub hot: f32,
    pub cool: f32,
}

impl Config {
    /// `None` when the governor is off (empty or `0`).
    pub fn parse(gov: &str, hot: Option<&str>, cool: Option<&str>) -> Result<Option<Self>, String> {
        let gov = gov.trim();
        if gov.is_empty() || gov == "0" {
            return Ok(None);
        }
        let mut steps = Vec::new();
        for word in gov.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            let step = match word {
                "scale" => Step::Scale,
                "cap" => Step::Cap,
                _ => {
                    return Err(format!(
                        "GRID_THERMAL_GOV: unknown step `{word}` (scale, cap)"
                    ));
                }
            };
            if !steps.contains(&step) {
                steps.push(step);
            }
        }
        let number = |value: Option<&str>, default: f32, name: &str| match value {
            None => Ok(default),
            Some(v) => v
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("{name}: not a number: `{v}`")),
        };
        let hot = number(hot, HOT, "GRID_THERMAL_HOT")?;
        let cool = number(cool, COOL, "GRID_THERMAL_COOL")?;
        if !hot.is_finite() || !cool.is_finite() || cool >= hot {
            return Err(format!(
                "GRID_THERMAL_COOL {cool} must be below GRID_THERMAL_HOT {hot}"
            ));
        }
        Ok(Some(Self { steps, hot, cool }))
    }

    fn from_env() -> Option<Self> {
        let var = |name| std::env::var(name).ok();
        match Self::parse(
            &var("GRID_THERMAL_GOV").unwrap_or_default(),
            var("GRID_THERMAL_HOT").as_deref(),
            var("GRID_THERMAL_COOL").as_deref(),
        ) {
            Ok(config) => config,
            Err(e) => {
                log_line!("thermal version=1 event=config_error error={e:?}");
                None
            }
        }
    }
}

/// The governor's level with its hysteresis timers.
#[derive(Debug, Default)]
pub struct Governor {
    pub level: usize,
    since_change: f32,
    cool_for: f32,
}

impl Governor {
    /// One sample, `dt` seconds after the previous one. A NaN headroom (no
    /// reading yet) is neither hot nor cool by itself. Returns the new level
    /// and the reason when the level changes.
    pub fn sample(
        &mut self,
        config: &Config,
        dt: f32,
        headroom: f32,
        status: i32,
    ) -> Option<(usize, &'static str)> {
        self.since_change += dt;
        let hot_headroom = headroom >= config.hot;
        let hot = hot_headroom || status >= HOT_STATUS;
        if headroom <= config.cool && status < HOT_STATUS {
            self.cool_for += dt;
        } else {
            self.cool_for = 0.;
        }
        if hot && self.level < config.steps.len() && self.since_change >= UP_DWELL_S {
            self.level += 1;
            self.since_change = 0.;
            return Some((self.level, if hot_headroom { "headroom" } else { "status" }));
        }
        if self.level > 0 && self.cool_for >= COOL_HOLD_S && self.since_change >= COOL_HOLD_S {
            self.level -= 1;
            self.since_change = 0.;
            self.cool_for = 0.;
            return Some((self.level, "cool"));
        }
        None
    }

    /// Whether `step` is active at the current level.
    pub fn active(&self, config: &Config, step: Step) -> bool {
        config.steps[..self.level.min(config.steps.len())].contains(&step)
    }
}

#[derive(Resource)]
struct State {
    config: Config,
    governor: Governor,
    samples: u64,
    last: Option<std::time::Instant>,
}

/// Starts the headroom thread (Android) and, with `GRID_THERMAL_GOV`, the
/// governor.
pub fn install(app: &mut App) {
    platform::start();
    if let Some(config) = Config::from_env() {
        log_line!(
            "thermal version=1 event=governor steps={:?} hot={} cool={} up_dwell_s={UP_DWELL_S} cool_hold_s={COOL_HOLD_S} scale_share={SCALE_SHARE} cap_hz={CAP_HZ} forecast_s={FORECAST_S}",
            config.steps,
            config.hot,
            config.cool
        );
        app.insert_resource(State {
            config,
            governor: Governor::default(),
            samples: 0,
            last: None,
        })
        .add_systems(PreUpdate, govern);
    }
}

fn govern(
    mut state: ResMut<State>,
    scale: Option<ResMut<crate::render_scale::RenderScale>>,
    #[cfg(target_os = "android")] display: Option<ResMut<crate::android_display::Display>>,
) {
    let (headroom, status, samples) = platform::read();
    if samples == state.samples {
        return;
    }
    state.samples = samples;
    let now = std::time::Instant::now();
    let dt = state
        .last
        .map_or(1., |t| now.duration_since(t).as_secs_f32());
    state.last = Some(now);
    let State {
        config, governor, ..
    } = &mut *state;
    let Some((level, reason)) = governor.sample(config, dt, headroom, status) else {
        return;
    };
    let scale_on = governor.active(config, Step::Scale);
    let cap_on = governor.active(config, Step::Cap);
    log_line!(
        "thermal version=1 event=level level={level} reason={reason} headroom={headroom:.3} status={status} scale={scale_on} cap={cap_on}"
    );
    LEVEL.store(level as u32, std::sync::atomic::Ordering::Relaxed);
    if let Some(mut scale) = scale {
        scale.thermal_share = scale_on.then_some(SCALE_SHARE);
    }
    #[cfg(target_os = "android")]
    if let Some(mut display) = display {
        display.set_thermal_cap(cap_on.then_some(CAP_HZ));
    }
}

static LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// ` thermal_h=.. thermal_status=.. thermal_level=..` for frame_stats
/// (Android; empty elsewhere).
pub fn window_report() -> String {
    if !cfg!(target_os = "android") {
        return String::new();
    }
    let (headroom, status, _) = platform::read();
    format!(
        " thermal_h={headroom:.3} thermal_status={status} thermal_level={}",
        LEVEL.load(std::sync::atomic::Ordering::Relaxed)
    )
}

#[cfg(target_os = "android")]
mod platform {
    use std::ffi::{c_int, c_void};
    use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

    static HEADROOM: AtomicU32 = AtomicU32::new(0x7fc0_0000); // NaN
    static STATUS: AtomicI32 = AtomicI32::new(-1);
    static SAMPLES: AtomicU64 = AtomicU64::new(0);

    type Acquire = unsafe extern "C" fn() -> *mut c_void;
    type CurrentStatus = unsafe extern "C" fn(*mut c_void) -> c_int;
    type Callback = unsafe extern "C" fn(*mut c_void, c_int);
    type Register = unsafe extern "C" fn(*mut c_void, Callback, *mut c_void) -> c_int;
    type Headroom = unsafe extern "C" fn(*mut c_void, c_int) -> f32;
    /// `AThermalHeadroomThreshold` (android/thermal.h, API 35).
    #[repr(C)]
    struct Threshold {
        headroom: f32,
        status: c_int,
    }
    type Thresholds = unsafe extern "C" fn(*mut c_void, *mut *const Threshold, *mut usize) -> c_int;

    unsafe extern "C" fn on_status(_: *mut c_void, status: c_int) {
        STATUS.store(status, Ordering::Relaxed);
    }

    pub fn read() -> (f32, i32, u64) {
        (
            f32::from_bits(HEADROOM.load(Ordering::Relaxed)),
            STATUS.load(Ordering::Relaxed),
            SAMPLES.load(Ordering::Relaxed),
        )
    }

    pub fn start() {
        let spawned = std::thread::Builder::new()
            .name("grid-thermal".into())
            .spawn(run);
        if let Err(e) = spawned {
            log_line!("thermal version=1 event=start result=no_thread error={e:?}");
        }
    }

    fn run() {
        // SAFETY: dlopen/dlsym with valid C strings; each symbol is cast to
        // its NDK signature (android/thermal.h). The manager is acquired once
        // and never released (it lives as long as the process); every call on
        // it is made from this thread, and the callback only stores an atomic.
        unsafe {
            let lib = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW);
            if lib.is_null() {
                log_line!("thermal version=1 event=start result=no_libandroid");
                return;
            }
            let sym = |name: &std::ffi::CStr| libc::dlsym(lib, name.as_ptr());
            let acquire = sym(c"AThermal_acquireManager");
            let current = sym(c"AThermal_getCurrentThermalStatus");
            let register = sym(c"AThermal_registerThermalStatusListener");
            let headroom = sym(c"AThermal_getThermalHeadroom");
            let thresholds = sym(c"AThermal_getThermalHeadroomThresholds");
            if acquire.is_null() || current.is_null() {
                log_line!("thermal version=1 event=start result=no_api");
                return;
            }
            let manager = std::mem::transmute::<*mut c_void, Acquire>(acquire)();
            if manager.is_null() {
                log_line!("thermal version=1 event=start result=no_manager");
                return;
            }
            let status = std::mem::transmute::<*mut c_void, CurrentStatus>(current)(manager);
            STATUS.store(status, Ordering::Relaxed);
            let listener = if register.is_null() {
                -1
            } else {
                std::mem::transmute::<*mut c_void, Register>(register)(
                    manager,
                    on_status,
                    std::ptr::null_mut(),
                )
            };
            let mut levels = String::new();
            if !thresholds.is_null() {
                let mut list: *const Threshold = std::ptr::null();
                let mut len = 0usize;
                let r = std::mem::transmute::<*mut c_void, Thresholds>(thresholds)(
                    manager, &mut list, &mut len,
                );
                if r == 0 && !list.is_null() {
                    for t in std::slice::from_raw_parts(list, len) {
                        levels.push_str(&format!("{}:{:.3},", t.status, t.headroom));
                    }
                    libc::free(list as *mut c_void);
                } else {
                    levels = format!("error{r}");
                }
            }
            log_line!(
                "thermal version=1 event=start result=ok status={status} listener={listener} headroom_api={} thresholds=[{}] forecast_s={}",
                !headroom.is_null(),
                levels.trim_end_matches(','),
                super::FORECAST_S
            );
            if headroom.is_null() {
                return;
            }
            let headroom = std::mem::transmute::<*mut c_void, Headroom>(headroom);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let h = headroom(manager, super::FORECAST_S);
                HEADROOM.store(h.to_bits(), Ordering::Relaxed);
                SAMPLES.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    pub fn start() {}

    pub fn read() -> (f32, i32, u64) {
        (f32::NAN, -1, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::parse("scale,cap", None, None).unwrap().unwrap()
    }

    #[test]
    fn config_parses_steps_and_thresholds() {
        assert_eq!(Config::parse("", None, None), Ok(None));
        assert_eq!(Config::parse("0", None, None), Ok(None));
        let c = Config::parse(" cap , scale,cap", Some("0.9"), Some("0.5"))
            .unwrap()
            .unwrap();
        assert_eq!(c.steps, vec![Step::Cap, Step::Scale]);
        assert_eq!((c.hot, c.cool), (0.9, 0.5));
        assert!(Config::parse("fast", None, None).is_err());
        assert!(Config::parse("cap", Some("0.5"), Some("0.6")).is_err());
        assert!(Config::parse("cap", Some("x"), None).is_err());
    }

    #[test]
    fn governor_raises_with_dwell_and_falls_after_cool_hold() {
        let c = config();
        let mut g = Governor::default();
        // Hot from the first sample: one raise per UP_DWELL_S, up to the step count.
        let mut changes = Vec::new();
        for i in 0..20 {
            if let Some(change) = g.sample(&c, 1., 0.85, 0) {
                changes.push((i, change));
            }
        }
        assert_eq!(changes, vec![(4, (1, "headroom")), (9, (2, "headroom"))]);
        assert!(g.active(&c, Step::Scale) && g.active(&c, Step::Cap));
        // Between the thresholds: no change, and the cool timer does not run.
        for _ in 0..100 {
            assert_eq!(g.sample(&c, 1., 0.7, 1), None);
        }
        // Cool, but a MODERATE status keeps it from counting.
        for _ in 0..40 {
            assert_eq!(g.sample(&c, 1., 0.3, 2).map(|c| c.0), None);
        }
        // Cool: one step down after COOL_HOLD_S, the next after another.
        let mut falls = Vec::new();
        for i in 0..70 {
            if let Some(change) = g.sample(&c, 1., 0.3, 0) {
                falls.push((i, change));
            }
        }
        assert_eq!(falls, vec![(29, (1, "cool")), (59, (0, "cool"))]);
        assert!(!g.active(&c, Step::Scale) && !g.active(&c, Step::Cap));
    }

    #[test]
    fn status_alone_is_hot_and_nan_headroom_is_neutral() {
        let c = config();
        let mut g = Governor::default();
        let mut changes = Vec::new();
        for i in 0..6 {
            if let Some(change) = g.sample(&c, 1., f32::NAN, 3) {
                changes.push((i, change));
            }
        }
        assert_eq!(changes, vec![(4, (1, "status"))]);
        assert!(g.active(&c, Step::Scale) && !g.active(&c, Step::Cap));
        for _ in 0..100 {
            assert_eq!(g.sample(&c, 1., f32::NAN, 0), None);
        }
    }
}
