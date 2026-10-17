//! Replay time control: play, pause, the rewind/fast-forward shuttle and the
//! end-of-replay loop (docs/REPLAY.md).
//!
//! From the original's replay controller [D]:
//! - 0x944430 advances replay time per frame by mode: play `shuttle x dt`,
//!   paused 0, scrub `+0xA84 x shuttle x dt` (+0xA84 starts at 1.0, 0x943FF0);
//! - 0x944710 reads the rewind and fast-forward actions: when either is above
//!   0.1, `f = (ff - 0.1)/0.9 - (rw - 0.1)/0.9`; playing, the shuttle is
//!   `3f + 1` (f >= 0) or `3f - 1` (f < 0), so a full key press runs at
//!   +/-4.33x; paused, a press scrubs with shuttle `f`; released, the shuttle
//!   returns to 1 and a scrub back to pause. Replay Pause toggles play/pause;
//! - seeks clamp to the oldest buffered time (0x91CC00);
//! - the flow's `loop` links: Instant Replay rewinds again
//!   (`c_pause_instant_race_replay` -> `..._hide_screen` -> rewind), the
//!   post-race replay begins again (`c_post_post_race_replay_loop`).

/// Rewind/fast-forward dead zone (0xCAFCD4) and scale 1/0.9 (0xCECA4C).
const DEAD_ZONE: f32 = 0.1;
const AXIS_SCALE: f32 = 1. / 0.9;
/// Shuttle gain while playing (0xCB1790).
const SHUTTLE_GAIN: f32 = 3.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Play,
    Pause,
    /// Paused with rewind or fast-forward held (mode 0xB).
    Scrub,
}

/// One frame of replay input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub pause: bool,
    /// Action values 0..1.
    pub rewind: f32,
    pub fast_forward: f32,
    pub next_lap: bool,
    pub previous_lap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Reached the end while playing forward: the flow loops.
    Loop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    /// Replay time, simulation seconds.
    pub time: f64,
    /// The readable window, simulation seconds.
    pub start: f64,
    pub end: f64,
    /// Where the loop restarts.
    pub loop_from: f64,
    pub mode: Mode,
    pub shuttle: f32,
}

impl Player {
    pub fn new(start: f64, end: f64, from: f64) -> Self {
        let from = from.clamp(start, end);
        Self {
            time: from,
            start,
            end,
            loop_from: from,
            mode: Mode::Play,
            shuttle: 1.,
        }
    }

    /// Lap markers (seconds): Next Lap jumps to the first after now,
    /// Previous Lap to the last more than a second before now (AUTHORED).
    pub fn step(&mut self, input: Input, dt: f64, laps: &[f64]) -> Option<Event> {
        if input.pause {
            self.mode = match self.mode {
                Mode::Play => Mode::Pause,
                Mode::Pause | Mode::Scrub => Mode::Play,
            };
            self.shuttle = 1.;
        }
        let held = input.rewind > DEAD_ZONE || input.fast_forward > DEAD_ZONE;
        let f = (input.fast_forward - DEAD_ZONE) * AXIS_SCALE - (input.rewind - DEAD_ZONE) * AXIS_SCALE;
        match (self.mode, held) {
            (Mode::Play, true) => {
                self.shuttle = if f < 0. {
                    f * SHUTTLE_GAIN - 1.
                } else {
                    f * SHUTTLE_GAIN + 1.
                };
            }
            (Mode::Play, false) => self.shuttle = 1.,
            (Mode::Pause, true) | (Mode::Scrub, true) => {
                self.mode = Mode::Scrub;
                self.shuttle = f;
            }
            (Mode::Scrub, false) => {
                self.mode = Mode::Pause;
                self.shuttle = 1.;
            }
            (Mode::Pause, false) => {}
        }
        if input.next_lap
            && let Some(&t) = laps.iter().find(|&&t| t > self.time + 1e-6 && t <= self.end)
        {
            self.time = t;
        }
        if input.previous_lap {
            let before = laps
                .iter()
                .rev()
                .find(|&&t| t < self.time - 1. && t >= self.start);
            self.time = before.copied().unwrap_or(self.start);
        }
        let rate = match self.mode {
            Mode::Pause => 0.,
            Mode::Play | Mode::Scrub => f64::from(self.shuttle),
        };
        let next = self.time + rate * dt;
        if next >= self.end && self.mode == Mode::Play && rate > 0. {
            self.time = self.loop_from;
            return Some(Event::Loop);
        }
        self.time = next.clamp(self.start, self.end);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(rewind: f32, fast_forward: f32) -> Input {
        Input {
            rewind,
            fast_forward,
            ..Default::default()
        }
    }

    #[test]
    fn the_shuttle_follows_the_original_controller() {
        let mut p = Player::new(0., 100., 10.);
        assert_eq!(p.step(Input::default(), 0.5, &[]), None);
        assert_eq!(p.time, 10.5);
        // Fast forward held fully: 3 x 1/0.9 + 1 = 4.333x.
        p.step(input(0., 1.), 1., &[]);
        assert!((p.shuttle - (3. / 0.9 + 1.)).abs() < 1e-5, "{}", p.shuttle);
        assert!((p.time - (10.5 + f64::from(p.shuttle))).abs() < 1e-9);
        // Rewind: -4.333x; released: back to 1x.
        p.step(input(1., 0.), 1., &[]);
        assert!((p.shuttle + (3. / 0.9 + 1.)).abs() < 1e-5);
        p.step(Input::default(), 0., &[]);
        assert_eq!(p.shuttle, 1.);
        // Below the dead zone nothing changes.
        p.step(input(0.1, 0.), 0., &[]);
        assert_eq!(p.shuttle, 1.);
    }

    #[test]
    fn pause_scrub_and_clamping() {
        let mut p = Player::new(0., 20., 5.);
        p.step(Input { pause: true, ..Default::default() }, 1., &[]);
        assert_eq!((p.mode, p.time), (Mode::Pause, 5.));
        // Paused, a held key scrubs at shuttle f.
        p.step(input(1., 0.), 1., &[]);
        assert_eq!(p.mode, Mode::Scrub);
        assert!((p.time - (5. - 1. / 0.9)).abs() < 1e-6);
        p.step(Input::default(), 1., &[]);
        assert_eq!(p.mode, Mode::Pause);
        // Rewinding past the start stops there.
        p.step(Input { pause: true, ..Default::default() }, 0., &[]);
        for _ in 0..10 {
            p.step(input(1., 0.), 1., &[]);
        }
        assert_eq!(p.time, 0.);
    }

    #[test]
    fn the_end_loops_and_laps_jump() {
        let mut p = Player::new(0., 20., 12.);
        assert_eq!(p.step(Input::default(), 9., &[]), Some(Event::Loop));
        assert_eq!(p.time, 12.);
        let laps = [1., 8., 15.];
        p.step(Input { next_lap: true, ..Default::default() }, 0., &laps);
        assert_eq!(p.time, 15.);
        p.step(Input { previous_lap: true, ..Default::default() }, 0., &laps);
        assert_eq!(p.time, 8.);
        p.step(Input { previous_lap: true, ..Default::default() }, 0., &laps);
        assert_eq!(p.time, 1.);
        p.step(Input { previous_lap: true, ..Default::default() }, 0., &laps);
        assert_eq!(p.time, 0.);
    }
}
