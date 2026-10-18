//! Demolition Derby rules (docs/DEMOLITION_DERBY.md).
//!
//! [V] from the executable (static analysis, F421):
//! - `gameconfig/demolition_derby.xml` is read by 0x630C30 into the derby
//!   manager's settings (manager 0x668820 at race +0x23E8, settings at +0x20).
//!   The keys and defaults (constructor 0x630BC0): `flippedCountdown` 5.0,
//!   `flippedThreshold` 1.0, `wrongWayThreshold` 1.0, `warningThreshold`
//!   0.5, `notMovingTreshold` 10.0 (mph) and `notMovingCountdown` 10.0 (s).
//!   The shipped file names `flippedVelocityThreshold`, which the reader
//!   does not read, so `flippedThreshold` keeps its default.
//! - Per car, every tick (0x630CF0 from the manager update 0x64DFF0): a car
//!   at or above `notMovingTreshold` (mph → m/s by 0xA7DDB0) restarts its
//!   countdown at `notMovingCountdown`; slower, the countdown runs down,
//!   and when it falls below zero an AI car (+0x6C != 0) is disqualified,
//!   0x9E64E0(6, 0). The countdown starts at 9999 (0x64E030), so a car that
//!   has not yet reached the threshold is never disqualified.
//! - The local player's still-and-tilted trigger (0x60D590, the auto-reset)
//!   disqualifies with 0x9E64E0(6, 0) instead of resetting when the car's
//!   +0x2E5C is 2 [I: the demolition mode].
//! - No reader of the flipped, wrong-way and warning settings was found; the
//!   `osd_demolition_dis` warning ("You will be disqualified!") is not ported.
use std::path::Path;

/// `gameconfig/demolition_derby.xml` (`DemolitionDerbySettings`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub flipped_countdown: f32,
    pub flipped_threshold: f32,
    pub wrong_way_threshold: f32,
    pub warning_threshold: f32,
    /// mph.
    pub not_moving_threshold: f32,
    /// Seconds.
    pub not_moving_countdown: f32,
}

impl Default for Settings {
    /// Constructor 0x630BC0.
    fn default() -> Self {
        Self {
            flipped_countdown: 5.,
            flipped_threshold: 1.,
            wrong_way_threshold: 1.,
            warning_threshold: 0.5,
            not_moving_threshold: 10.,
            not_moving_countdown: 10.,
        }
    }
}

impl Settings {
    pub fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join("gameconfig/demolition_derby.xml"))?;
        let doc = grid_formats::binxml::read(&bytes)?;
        let e = doc.elements.first().ok_or("demolition_derby.xml empty")?;
        if e.name != "DemolitionDerbySettings" {
            return Err("expected DemolitionDerbySettings".into());
        }
        let mut s = Self::default();
        for (key, value) in &e.attributes {
            let field = match *key {
                "flippedCountdown" => &mut s.flipped_countdown,
                "flippedThreshold" => &mut s.flipped_threshold,
                "wrongWayThreshold" => &mut s.wrong_way_threshold,
                "warningThreshold" => &mut s.warning_threshold,
                "notMovingTreshold" => &mut s.not_moving_threshold,
                "notMovingCountdown" => &mut s.not_moving_countdown,
                // Not read by the original (0x630C30), e.g. flippedVelocityThreshold.
                _ => continue,
            };
            let v: f32 = value.parse()?;
            if !v.is_finite() {
                return Err(format!("demolition_derby.xml {key} not finite").into());
            }
            *field = v;
        }
        Ok(s)
    }

    /// `notMovingTreshold` in m/s (0xA7DDB0: × 1.609344 / 3.6).
    pub fn not_moving_speed(&self) -> f32 {
        self.not_moving_threshold * 1.609_344 / 3.6
    }
}

/// One car's not-moving countdown (derby record +0x24).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotMoving {
    countdown: f32,
}

impl Default for NotMoving {
    /// The record reset 0x64E030.
    fn default() -> Self {
        Self { countdown: 9999. }
    }
}

impl NotMoving {
    /// One tick at `speed` m/s; true when the car is to be disqualified
    /// (the caller applies it to AI cars only, 0x630CF0).
    pub fn tick(&mut self, settings: &Settings, speed: f32, dt: f64) -> bool {
        if settings.not_moving_speed() <= speed {
            self.countdown = settings.not_moving_countdown;
            false
        } else {
            self.countdown -= dt as f32;
            self.countdown < 0.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn the_shipped_settings_keep_the_unread_flipped_threshold_default() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let s = Settings::load(Path::new(&root)).unwrap();
        assert_eq!(
            s,
            Settings {
                wrong_way_threshold: 50.,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn a_stopped_car_is_disqualified_after_the_countdown_once_it_has_driven() {
        let s = Settings::default();
        let dt = 1. / 60.;
        let mut n = NotMoving::default();
        // Waiting on the grid: never.
        for _ in 0..6000 {
            assert!(!n.tick(&s, 0., dt));
        }
        // Driving at 5 m/s (above 10 mph = 4.47 m/s) resets the countdown.
        let mut n = NotMoving::default();
        assert!(!n.tick(&s, 5., dt));
        // Then stopped: disqualified after just over 10 s.
        let mut ticks = 0;
        while !n.tick(&s, 1., dt) {
            ticks += 1;
            assert!(ticks < 700);
        }
        // 10 s of f32 countdown at 1/60 s (rounding decides the last tick).
        assert!((599..=601).contains(&ticks), "{ticks}");
    }
}
