//! Refresh-divisor frame pacing (F958 trace, docs/PERFORMANCE.md "Refresh
//! pacing").
//!
//! Under vsync (Fifo) a frame longer than one refresh period waits for the
//! next one, so on a fast display (a 120 Hz ProMotion panel, 144/165 Hz
//! monitors) 10-15 ms frames land on 8.3, 16.7 and 25 ms intervals: a
//! steady 60 looks smoother than an uneven 70-90 (F958, the owner's
//! MacBook). This module reads the display's refresh rate at run time
//! (winit's current monitor, read again each second, so a window moved to
//! another screen is followed; the Display on Android) and paces frames to
//! the highest rate R/n that divides the refresh evenly, is at least
//! [`MIN_HZ`] and that the measured frame time sustains with a margin:
//! 120 -> 60, 144 -> 72, 240 -> 120/80/60. Where none fits (a 60, 75 or
//! 100 Hz display, a frame too slow for R/2), frames run at plain vsync as
//! before; no rate below about 60 FPS is chosen for evenness. Moves are
//! held for seconds with separate margins up and down ([`Pacer`]), and a
//! rate that did not hold (plain vsync too) waits longer each time before
//! it is tried again.
//!
//! A fixed cap (Android `[video] refresh_hz`, the thermal governor; F378,
//! F935) still applies; the lower rate wins. wgpu gives no control of the
//! present time or a preferred frame rate (Metal's
//! `presentAfterMinimumDuration`, CADisplayLink's preferred frame rate
//! range), so the pace is a sleep at the end of the main schedule, the
//! mechanism the Android cap has used since F378; a ProMotion panel
//! follows the steady cadence.
//!
//! `[video] refresh_pacing = auto | on | off`: `auto` paces on macOS, iOS
//! and Android and leaves Windows and Linux at plain vsync (variable-refresh
//! monitors; the owner's 165 Hz PC runs 100+ FPS uncapped). Pacing applies
//! only to the vsync present modes (`fifo`, `auto_vsync`).
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use std::time::{Duration, Instant};

/// The lowest paced rate: never below about 60 FPS for evenness (59.94 Hz
/// displays and their halves of 119.88 count as 60).
pub const MIN_HZ: f32 = 59.5;
/// From plain vsync: the mean frame interval times this fits the paced
/// period. Vsync rounds every frame up to whole refresh periods (and the
/// owner's Mac waits for drawables beyond its own work, Metal traces of 2026-11-26), so the mean
/// already overstates the frame and needs no further margin.
pub const ENGAGE: f32 = 1.0;
/// Plain vsync keeps up while the mean frame is within this share of the
/// refresh period.
pub const KEEPS_UP: f32 = 1.1;
/// A pace is late when the 90th percentile frame time without the pacing
/// sleep exceeds its period by this share (about dynres's 18 ms at 60).
pub const OVERRUN: f32 = 1.1;
/// A slower pace is chosen when the 90th percentile times this fits it.
pub const SLOWER: f32 = 1.1;
/// A faster pace (or plain vsync) when the 90th percentile times this fits.
pub const FASTER: f32 = 1.25;
/// Seconds a move to a slower pace waits, and a move to a faster one.
pub const DOWN_SECONDS: u32 = 2;
pub const UP_SECONDS: u32 = 5;
/// Seconds before a rate (plain vsync included) that did not hold is tried
/// again; doubles for that rate each time up to [`MAX_BACKOFF_SECONDS`].
pub const BACKOFF_SECONDS: u32 = 20;
pub const MAX_BACKOFF_SECONDS: u32 = 160;
/// Measurement tolerance of a period comparison, ms.
const EPS_MS: f32 = 0.05;
/// Frames longer than this (loading, a pause) are left out.
const HITCH_MS: f32 = 250.;

/// `[video] refresh_pacing`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// On for macOS, iOS and Android; off on Windows and Linux.
    #[default]
    Auto,
    On,
    Off,
}

impl Mode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => Err("`refresh_pacing` is auto, on or off".into()),
        }
    }

    pub fn enabled(self) -> bool {
        match self {
            Self::Auto => cfg!(any(target_os = "macos", target_os = "ios", target_os = "android")),
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// The divisors `n` a display of `refresh_hz` offers, fastest first: 1
/// (plain vsync) and every `n` with `refresh_hz / n` at least [`MIN_HZ`].
pub fn divisors(refresh_hz: f32) -> impl Iterator<Item = u32> {
    (1..=8u32).filter(move |&n| n == 1 || refresh_hz / n as f32 >= MIN_HZ)
}

/// The pace decision, one call per second of frames.
///
/// A rate holds while the mean frame interval stays within [`KEEPS_UP`] of
/// its period and, when paced, the 90th percentile frame time without the
/// pacing sleep within [`OVERRUN`]. A rate that does not hold for
/// [`DOWN_SECONDS`] is left for the fastest slower rate the frame fits
/// (plain vsync when none does) and blocked for [`BACKOFF_SECONDS`],
/// doubling each time. On the owner's Mac (Metal traces of 2026-11-26) the frame's own work fits
/// 120 Hz but vsync still delivers a 15 ms mean, so once paced at 60 the
/// work alone would try 120 again every few seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pacer {
    refresh_hz: f32,
    /// The divisor in use; 1 is plain vsync.
    pub n: u32,
    late: u32,
    early: u32,
    /// Seconds each divisor stays blocked (index: divisor).
    blocked: [u32; 9],
    /// The block each divisor gets the next time it does not hold.
    block_len: [u32; 9],
}

impl Pacer {
    fn period(&self, n: u32) -> f32 {
        1000. * n as f32 / self.refresh_hz
    }

    /// `ms` fits a pace of divisor `n`.
    fn fits(&self, n: u32, ms: f32) -> bool {
        ms <= self.period(n) + EPS_MS
    }

    fn block(&mut self, n: u32) {
        let n = n as usize;
        self.block_len[n] = (self.block_len[n] * 2).clamp(BACKOFF_SECONDS, MAX_BACKOFF_SECONDS);
        self.blocked[n] = self.block_len[n];
    }

    fn open(&self, n: u32) -> bool {
        self.blocked[n as usize] == 0
    }

    /// One second at `refresh_hz`: `mean_ms` is the mean frame interval,
    /// `work_ms` the 90th percentile frame time without the pacing sleep.
    /// Returns the divisor to pace with (1: plain vsync).
    pub fn second(&mut self, refresh_hz: f32, mean_ms: f32, work_ms: f32) -> u32 {
        if (refresh_hz - self.refresh_hz).abs() > 0.5 || self.n == 0 {
            // A new display (or the first second): plain vsync, measured anew.
            *self = Self { refresh_hz, n: 1, ..Self::default() };
            return 1;
        }
        for b in &mut self.blocked {
            *b = b.saturating_sub(1);
        }
        let n = self.n;
        let holds = self.fits(n, mean_ms / KEEPS_UP) && (n == 1 || self.fits(n, work_ms / OVERRUN));
        if !holds {
            self.early = 0;
            // At vsync a frame measures whole periods, so the mean is the
            // measure; paced, the frame's own time without the sleep.
            let need = if n == 1 { mean_ms * ENGAGE } else { work_ms * SLOWER };
            let slower = divisors(refresh_hz).find(|&m| m > n && self.open(m) && self.fits(m, need));
            // Frames slower than every even pace fall back to plain vsync.
            match slower.or((n > 1).then_some(1)) {
                Some(m) => {
                    self.late += 1;
                    if self.late >= DOWN_SECONDS {
                        self.late = 0;
                        self.block(n);
                        self.n = m;
                    }
                }
                None => self.late = 0,
            }
            return self.n;
        }
        self.late = 0;
        match divisors(refresh_hz).find(|&m| m < n && self.open(m) && self.fits(m, work_ms * FASTER)) {
            Some(m) => {
                self.early += 1;
                if self.early >= UP_SECONDS {
                    self.n = m;
                    self.early = 0;
                }
            }
            None => self.early = 0,
        }
        self.n
    }
}

/// The pacing state of one app (the race, the menu).
#[derive(Resource)]
pub struct RefreshPace {
    enabled: bool,
    /// The fixed cap (Android `refresh_hz` with the thermal cap).
    cap_hz: Option<f32>,
    pacer: Pacer,
    refresh_hz: Option<f32>,
    due: Option<Instant>,
    last: Option<Instant>,
    slept: Duration,
    second: Option<Instant>,
    intervals: Vec<f32>,
    work: Vec<f32>,
}

impl RefreshPace {
    pub fn new(mode: Mode, cap_hz: Option<f32>) -> Self {
        Self {
            enabled: mode.enabled(),
            cap_hz,
            pacer: Pacer::default(),
            refresh_hz: None,
            due: None,
            last: None,
            slept: Duration::ZERO,
            second: None,
            intervals: Vec::with_capacity(256),
            work: Vec::with_capacity(256),
        }
    }

    /// A fixed cap (`None`: none); the lower of it and the pace applies.
    /// Called by the Android frame cap (android_display.rs) only.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_cap(&mut self, hz: Option<f32>) {
        self.cap_hz = hz;
    }

    /// Pacing is on (`[video] refresh_pacing`, by platform for `auto`).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The rate frames are held to: the lower of the cap and the pace,
    /// `None` at plain vsync.
    pub fn target_hz(&self) -> Option<f32> {
        match (self.cap_hz, self.pace_hz()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The paced rate, `None` at plain vsync.
    pub fn pace_hz(&self) -> Option<f32> {
        let n = self.pacer.n;
        if n > 1 { self.refresh_hz.map(|r| r / n as f32) } else { None }
    }
}

/// The refresh rate of the screen the window is on, `None` if unknown.
fn refresh_hz() -> Option<f32> {
    #[cfg(target_os = "android")]
    if let Some(hz) = crate::android::display_refresh_hz() {
        return Some(hz);
    }
    bevy::winit::WINIT_WINDOWS
        .try_with(|w| {
            let w = w.try_borrow().ok()?;
            // macOS can report no current monitor at window creation
            // (`Can't select current monitor`, F958 traces): the primary one.
            let window = w.windows.values().next()?;
            let monitor = window.current_monitor().or_else(|| window.primary_monitor())?;
            monitor.refresh_rate_millihertz().map(|m| m as f32 / 1000.)
        })
        .ok()
        .flatten()
        .filter(|hz| (20. ..=1000.).contains(hz))
}

fn percentile(values: &mut [f32], p: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * p).round() as usize]
}

/// Measures the frame, decides once a second and sleeps to the pace (`Last`,
/// on the main thread for winit's monitor).
fn pace(
    mut state: ResMut<RefreshPace>,
    windows: Query<&Window, With<PrimaryWindow>>,
    _main: NonSendMarker,
) {
    let state = &mut *state;
    let now = Instant::now();
    if let Some(last) = state.last {
        let interval = (now - last).as_secs_f32() * 1000.;
        if interval < HITCH_MS {
            state.intervals.push(interval);
            state.work.push(interval - state.slept.as_secs_f32() * 1000.);
        }
    }
    state.last = Some(now);
    let start = *state.second.get_or_insert(now);
    if now - start >= Duration::from_secs(1) {
        state.second = Some(now);
        let refresh = refresh_hz();
        if refresh.map(f32::round) != state.refresh_hz.map(f32::round) {
            println!("refresh_pace display_hz={refresh:?} was={:?} enabled={}", state.refresh_hz, state.enabled);
        }
        state.refresh_hz = refresh;
        let vsync = windows.iter().next().is_none_or(|w| {
            matches!(w.present_mode, PresentMode::Fifo | PresentMode::FifoRelaxed | PresentMode::AutoVsync)
        });
        let before = state.pace_hz();
        match refresh {
            Some(hz) if state.enabled && vsync && !state.intervals.is_empty() => {
                let mean = state.intervals.iter().sum::<f32>() / state.intervals.len() as f32;
                let work = percentile(&mut state.work, 0.9);
                state.pacer.second(hz, mean, work);
                let after = state.pace_hz();
                if after != before {
                    println!(
                        "refresh_pace display_hz={hz:.2} pace_hz={} mean_ms={mean:.2} work_p90_ms={work:.2} cap_hz={:?}",
                        after.map_or("vsync".into(), |p| format!("{p:.2}")),
                        state.cap_hz
                    );
                }
            }
            _ => state.pacer = Pacer::default(),
        }
        state.intervals.clear();
        state.work.clear();
    }
    let Some(hz) = state.target_hz() else {
        state.due = None;
        state.slept = Duration::ZERO;
        return;
    };
    let period = Duration::from_secs_f32(1. / hz);
    let (sleep, next) = crate::render_scale::pace(now, state.due, period);
    if !sleep.is_zero() {
        std::thread::sleep(sleep);
    }
    state.slept = sleep;
    state.due = Some(next);
}

/// Paces this app's frames (`mode` from `[video] refresh_pacing`, `cap_hz`
/// the fixed cap: Android's `refresh_hz`).
pub fn install(app: &mut App, mode: Mode, cap_hz: Option<f32>) {
    println!("refresh_pace mode={mode:?} enabled={} cap_hz={cap_hz:?}", mode.enabled());
    app.insert_resource(RefreshPace::new(mode, cap_hz)).add_systems(Last, pace);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `seconds` of identical seconds and returns the paced rates.
    fn run(p: &mut Pacer, refresh: f32, seconds: u32, mean: f32, work: f32) -> f32 {
        for _ in 0..seconds {
            p.second(refresh, mean, work);
        }
        refresh / p.n as f32
    }

    #[test]
    fn divisors_stay_at_or_above_60() {
        let d = |hz: f32| divisors(hz).collect::<Vec<_>>();
        assert_eq!(d(60.), [1]);
        assert_eq!(d(59.94), [1]);
        assert_eq!(d(75.), [1]);
        assert_eq!(d(100.), [1]);
        assert_eq!(d(119.88), [1, 2]);
        assert_eq!(d(120.), [1, 2]);
        assert_eq!(d(144.), [1, 2]);
        assert_eq!(d(165.), [1, 2]);
        assert_eq!(d(240.), [1, 2, 3, 4]);
    }

    /// The pace each display settles at for a steady frame of `ms`,
    /// starting from plain vsync, which rounds the frame up to whole
    /// periods.
    fn settle(refresh: f32, ms: f32) -> f32 {
        let mut p = Pacer::default();
        let period = 1000. / refresh;
        let vsync = (ms / period).ceil() * period;
        run(&mut p, refresh, 10, vsync, vsync);
        // Paced: the frame time without the sleep is the frame's own.
        let mean = if p.n == 1 { vsync } else { vsync.max(p.period(p.n)) };
        run(&mut p, refresh, 10, mean, ms)
    }

    #[test]
    fn the_highest_even_rate_the_frame_sustains() {
        // 120 Hz ProMotion: 10-16 ms frames pace to 60; 7 ms keep 120.
        assert_eq!(settle(120., 7.), 120.);
        assert_eq!(settle(120., 11.), 60.);
        assert_eq!(settle(120., 13.5), 60.);
        assert_eq!(settle(120., 15.5), 60.);
        // 144 Hz: 9-13 ms frames pace to 72.
        assert_eq!(settle(144., 10.), 72.);
        assert_eq!(settle(144., 13.), 72.);
        assert_eq!(settle(144., 5.), 144.);
        // 165 Hz: 9 ms to 82.5; the owner's PC-class 6 ms stays at 165.
        assert_eq!(settle(165., 9.), 82.5);
        assert_eq!(settle(165., 5.5), 165.);
        // 240 Hz: the fastest even rate that fits.
        assert_eq!(settle(240., 6.), 120.);
        assert_eq!(settle(240., 10.), 80.);
        assert_eq!(settle(240., 13.), 60.);
        // 60, 75 and 100 Hz: plain vsync whatever the frame (no 30 FPS cap).
        for refresh in [60., 75., 100.] {
            for ms in [5., 12., 20., 30.] {
                assert_eq!(settle(refresh, ms), refresh, "{refresh} Hz, {ms} ms");
            }
        }
        // Too slow for any even rate at or above 60: plain vsync.
        assert_eq!(settle(120., 17.5), 120.);
        assert_eq!(settle(144., 15.), 144.);
    }

    /// The owner's Mac (Metal traces of 2026-11-26): the frame's work fits 120 Hz, but
    /// vsync delivers a 15 ms mean. Pacing at 60 holds; 120 is retried
    /// after 20 s, then 40 s, not every few seconds.
    #[test]
    fn presentation_limited_vsync_paces_and_retries_back_off() {
        let mut p = Pacer::default();
        assert_eq!(run(&mut p, 120., 3, 15.3, 15.3), 60.);
        assert_eq!(run(&mut p, 120., 23, 16.7, 6.), 60.);
        assert_eq!(run(&mut p, 120., 1, 16.7, 6.), 120.);
        assert_eq!(run(&mut p, 120., 2, 15.3, 15.3), 60.);
        assert_eq!(run(&mut p, 120., 43, 16.7, 6.), 60.);
        assert_eq!(run(&mut p, 120., 1, 16.7, 6.), 120.);
    }

    #[test]
    fn moves_wait_and_do_not_flip() {
        let mut p = Pacer::default();
        // One slow second does not engage; two do.
        p.second(120., 8.4, 8.4);
        assert_eq!(p.second(120., 12., 12.), 1);
        assert_eq!(p.second(120., 8.4, 8.4), 1);
        assert_eq!(p.second(120., 12., 12.), 1);
        assert_eq!(p.second(120., 12., 12.), 2);
        // Paced at 60, a 7.5 ms frame stays (7.5 x 1.25 > 8.33)...
        assert_eq!(run(&mut p, 120., 30, 16.7, 7.5), 60.);
        // ... 6 ms returns to 120 after five seconds, not before.
        assert_eq!(run(&mut p, 120., 4, 16.7, 6.), 60.);
        assert_eq!(run(&mut p, 120., 1, 16.7, 6.), 120.);
        // At 120 the frame measures one period and keeps up: no flip back.
        assert_eq!(run(&mut p, 120., 60, 8.4, 8.4), 120.);
    }

    #[test]
    fn a_pace_that_does_not_hold_backs_off() {
        let mut p = Pacer::default();
        assert_eq!(run(&mut p, 120., 3, 12., 12.), 60.);
        // Frames grow past 60 (18.3 ms): back to vsync after two seconds.
        assert_eq!(run(&mut p, 120., 1, 20., 19.), 60.);
        assert_eq!(run(&mut p, 120., 1, 20., 19.), 120.);
        // Light again, but the back-off holds plain vsync for 20 s...
        assert_eq!(run(&mut p, 120., 19, 12., 12.), 120.);
        assert_eq!(run(&mut p, 120., 3, 12., 12.), 60.);
        // ... and twice as long after the next failure.
        assert_eq!(run(&mut p, 120., 2, 20., 19.), 120.);
        assert_eq!(run(&mut p, 120., 39, 12., 12.), 120.);
        assert_eq!(run(&mut p, 120., 3, 12., 12.), 60.);
    }

    #[test]
    fn a_new_display_starts_at_vsync() {
        let mut p = Pacer::default();
        assert_eq!(run(&mut p, 120., 3, 12., 12.), 60.);
        // Moved to a 60 Hz screen: plain vsync, nothing slower.
        assert_eq!(run(&mut p, 60., 10, 16.7, 12.), 60.);
        assert_eq!(p.n, 1);
        // And to a 144 Hz one: measured anew, then 72.
        assert_eq!(run(&mut p, 144., 1, 12., 12.), 144.);
        assert_eq!(run(&mut p, 144., 2, 11., 11.), 72.);
    }

    #[test]
    fn mode_parse_and_platform_default() {
        assert_eq!(Mode::parse("auto"), Ok(Mode::Auto));
        assert_eq!(Mode::parse("on"), Ok(Mode::On));
        assert_eq!(Mode::parse("off"), Ok(Mode::Off));
        assert!(Mode::parse("120").is_err());
        assert!(Mode::On.enabled() && !Mode::Off.enabled());
        assert_eq!(Mode::Auto.enabled(), cfg!(any(target_os = "macos", target_os = "ios", target_os = "android")));
    }
}
