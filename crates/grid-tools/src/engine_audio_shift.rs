//! Gear-shift effects of the engine-audio update 451770 (docs/AUDIO_RESEARCH.md
//! §20): the gear change 414750 (direction, timer, RPM dip, wobble loudness),
//! the load offset of 451580 (controller +0x1A0) and the pitch wobble of
//! 4150D0 (+0x2F8). The controller is the car-audio object +0x8660; its
//! property block (+0x118) is the car-audio object +0x1190.

/// engine.ini `[Physics]` keys of the shift effects, read by 423BD6 through
/// 416040 (a case-insensitive key match: 11 shipped files spell
/// `fGearupShiftTime`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// fGearUpShiftTime: controller +0x80.
    pub up_shift_time: f32,
    /// fGearDownShiftTime: +0x94.
    pub down_shift_time: f32,
    /// GearUpWobbleLevel: property +0x7D20, the loudness until the first
    /// gear change (constructor 4223xx).
    pub up_wobble_level: f32,
    /// GearUpWobbleFadeOutTime, GearUpWobbleRate: +0x7D24, +0x7D28.
    pub up_wobble_fade: f32,
    pub up_wobble_rate: f32,
    /// fGearDownWobbleLoudness, Rate, Length: +0x7D34, +0x7D38, +0x7D3C.
    pub down_wobble_loudness: f32,
    pub down_wobble_rate: f32,
    pub down_wobble_length: f32,
    /// fUpWobbleStartDelay, fDownWobbleStartDelay: +0x7D40, +0x7D44.
    pub up_wobble_delay: f32,
    pub down_wobble_delay: f32,
    /// fWobbleRandomDown, fWobbleRandomUp (percent): +0x7D48, +0x7D4C.
    pub wobble_random_down: f32,
    pub wobble_random_up: f32,
}

impl Settings {
    /// No shift effects (synthetic banks).
    pub const NONE: Self = Self {
        up_shift_time: 0.,
        down_shift_time: 0.,
        up_wobble_level: 0.,
        up_wobble_fade: 0.,
        up_wobble_rate: 0.,
        down_wobble_loudness: 0.,
        down_wobble_rate: 0.,
        down_wobble_length: 0.,
        up_wobble_delay: 0.,
        down_wobble_delay: 0.,
        wobble_random_down: 0.,
        wobble_random_up: 0.,
    };
    pub fn read(ini: &str) -> Result<Self, &'static str> {
        let value = |key: &str| -> Result<f32, &'static str> {
            let mut section = "";
            let mut found = None;
            for line in ini.lines().map(str::trim) {
                if line.starts_with('[') && line.ends_with(']') {
                    section = &line[1..line.len() - 1];
                } else if section == "Physics"
                    && let Some((name, text)) = line.split_once('=')
                    && name.eq_ignore_ascii_case(key)
                {
                    if found.is_some() {
                        return Err("duplicate engine shift key");
                    }
                    let v = text.trim().parse::<f32>().map_err(|_| "invalid engine shift number")?;
                    if !v.is_finite() || !(0.0..=100.).contains(&v) {
                        return Err("engine shift value range");
                    }
                    found = Some(v);
                }
            }
            found.ok_or("missing engine shift key")
        };
        Ok(Self {
            up_shift_time: value("fGearUpShiftTime")?,
            down_shift_time: value("fGearDownShiftTime")?,
            up_wobble_level: value("GearUpWobbleLevel")?,
            up_wobble_fade: value("GearUpWobbleFadeOutTime")?,
            up_wobble_rate: value("GearUpWobbleRate")?,
            down_wobble_loudness: value("fGearDownWobbleLoudness")?,
            down_wobble_rate: value("fGearDownWobbleRate")?,
            down_wobble_length: value("fGearDownWobbleLength")?,
            up_wobble_delay: value("fUpWobbleStartDelay")?,
            down_wobble_delay: value("fDownWobbleStartDelay")?,
            wobble_random_down: value("fWobbleRandomDown")?,
            wobble_random_up: value("fWobbleRandomUp")?,
        })
    }
}

/// Property +0x7D18 (car-audio +0x8EA8): set to 1.0 by 42F770 and never
/// written again, so (2·r − 1) is 1 and the random terms are fixed.
const RANDOM: f32 = 1.;
/// Gears at vehicle +0x196C: 0 neutral, 1.. forward, 9 reverse.
const NEUTRAL: u32 = 0;
const FIRST: u32 = 1;
const REVERSE: u32 = 9;

/// The shift state of one engine's controller.
#[derive(Clone, Debug)]
pub struct Shift {
    settings: Settings,
    /// +0: scale of the physical RPM ratio read by 422A40 next frame.
    scale: f32,
    /// +4: the last counted change went up.
    up: bool,
    /// +0xC: seconds since it.
    elapsed: f32,
    /// +0x18: the gear last seen (0 from the constructor).
    gear: u32,
    /// +0x314: wobble loudness.
    loudness: f32,
    /// +0x1A0: load offset.
    load: f32,
    /// +0x310: a counted change not yet ended by the clunk (414750 sets it
    /// for the player only; 451580 clears it).
    clunk: bool,
    random: u32,
}

/// What 451770 publishes for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    /// +0: the RPM ratio scale for the next frame's 422A40.
    pub scale: f32,
    /// +0x1A0: added to the load crossfade phase by 42DE60.
    pub load: f32,
    /// +0x2F8: multiplies every voice pitch in 42DE60.
    pub pitch: f32,
    /// 451580 calls 4511E0 this frame: the gear clunk (player only).
    pub clunk: bool,
}

impl Shift {
    /// The constructor's state; `seed` stands in for the original's random
    /// generator (A682A0) [I: its sequence is not reproduced].
    pub fn new(settings: Settings, seed: u32) -> Self {
        Self {
            settings,
            scale: 1.,
            up: false,
            elapsed: 0.,
            gear: NEUTRAL,
            loudness: settings.up_wobble_level,
            load: 0.,
            clunk: false,
            random: seed | 1,
        }
    }

    /// +4 and +0xC: the last counted change went up, seconds since it (0 on
    /// the frame of the change).
    pub fn since(&self) -> (bool, f32) {
        (self.up, self.elapsed)
    }

    fn next_random(&mut self) -> u32 {
        // xorshift32 [I].
        let mut x = self.random;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.random = x;
        x
    }

    /// One frame of `delta` seconds in `gear`, after 422A40 (451770 order:
    /// the timer, 414750, 451580, 4150D0).
    pub fn frame(&mut self, gear: u32, delta: f32) -> Effects {
        let s = self.settings;
        self.elapsed += delta.abs();
        // 414750: the dip recovers by 0.003 per frame.
        if self.scale < 1. {
            self.scale = (self.scale + 0.003).min(1.);
        }
        if gear != self.gear {
            let up = gear > self.gear;
            self.loudness = s.down_wobble_loudness * (1. - gear as f32 * (1. / 6.)).clamp(0., 1.);
            self.scale = if up {
                (self.next_random() % 1000) as f32 * 5e-5 + 0.95
            } else {
                1.
            };
            // A change between neutral, first and reverse has no effect.
            let between = |g: u32| u32::from(gear == g || self.gear == g);
            if between(FIRST) + between(NEUTRAL) + between(REVERSE) < 2 {
                self.up = up;
                self.elapsed = 0.;
                self.clunk = true;
            }
            self.gear = gear;
        }
        // 451580: the offset decays by 0.9 per frame within [-1, 1]; an
        // upshift pulls it down by 0.5 a frame for fGearUpShiftTime (the
        // load crossfade to off-load), a downshift up by 0.1 a frame.
        let decayed = self.load * 0.9;
        self.load = if decayed >= -1. { decayed.min(1.) } else { -1. };
        let mut clunk = false;
        if self.up {
            if self.elapsed < s.up_shift_time {
                self.load -= 0.5;
            } else if self.clunk {
                // The upshift has ended: 4511E0 plays GearClucks.
                clunk = true;
                self.clunk = false;
            }
        } else if self.elapsed < s.down_shift_time {
            self.load += 0.1;
        }
        // 4150D0: a decaying sine on the pitch after the start delay.
        let mut pitch = 1.;
        let tau = std::f32::consts::TAU;
        if self.up {
            let t = self.elapsed - s.up_wobble_delay;
            if self.elapsed > s.up_wobble_delay && t < s.up_wobble_fade {
                let rate = ((RANDOM * 2. - 1.) * s.wobble_random_up * 0.01 + 1.) * s.up_wobble_rate;
                let sine = f64::from(rate * t * tau).sin() as f32;
                pitch = sine * ((1. - t / s.up_wobble_fade) * self.loudness) + 1.;
            }
        } else {
            let t = self.elapsed - s.down_wobble_delay;
            if self.elapsed > s.down_wobble_delay && t < s.down_wobble_length {
                let sine = f64::from(s.down_wobble_rate * t * tau).sin() as f32;
                let random = (RANDOM * 2. - 1.) * s.wobble_random_down * 0.01 + 1.;
                pitch = sine * (random * ((1. - t / s.down_wobble_length) * self.loudness)) + 1.;
            }
        }
        Effects { scale: self.scale, load: self.load, pitch, clunk }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LACETTI: &str = "[Physics]\n\
        GearUpWobbleFadeOutTime=0.25\nGearUpWobbleLevel=0.07\nGearUpWobbleRate=5.00\n\
        fGearDownShiftTime=0.100000\nfGearupShiftTime=0.13\nfGearDownWobbleLoudness=0.150000\n\
        fGearDownWobbleRate=15.000000\nfGearDownWobbleLength=0.300000\nfWobbleRandomDown=75.000000\n\
        fWobbleRandomUp=75.000000\nfUpWobbleStartDelay=0.18\nfDownWobbleStartDelay=0.000000\n[Idle]\n";

    fn lacetti() -> Settings {
        Settings::read(LACETTI).unwrap()
    }

    #[test]
    fn keys_match_without_case_and_only_in_physics() {
        let s = lacetti();
        assert_eq!((s.up_shift_time, s.down_wobble_rate, s.up_wobble_delay), (0.13, 15., 0.18));
        assert!(Settings::read(&LACETTI.replace("[Physics]", "[Idle]")).is_err());
        assert!(Settings::read(&format!("{LACETTI}[Physics]\nGearUpWobbleRate=4\n")).is_err());
    }

    #[test]
    fn an_upshift_dips_the_rpm_crossfades_off_load_and_wobbles_after_the_delay() {
        let mut shift = Shift::new(lacetti(), 7);
        let dt = 0.016;
        // Neutral to first at the start: no effect apart from the
        // constructor's state (a downshift-like start, +4 clear).
        let mut start = Vec::new();
        for _ in 0..40 {
            start.push(shift.frame(FIRST, dt));
        }
        assert_eq!(start[39], Effects { scale: 1., load: 0., pitch: 1., clunk: false }.with_load(start[39].load));
        let up = shift.frame(2, dt);
        assert!((0.95..1.).contains(&up.scale), "{up:?}");
        // 0.9·previous − 0.5 for 0.13 s: off-load (the phase is clamped).
        assert!(up.load < -0.48, "{up:?}");
        let frames: Vec<_> = (0..40).map(|_| shift.frame(2, dt)).collect();
        // −1.5 from the third frame to the end of the 0.13 s, then the clamp.
        assert!(frames[2..8].iter().all(|f| f.load == -1.5), "{frames:?}");
        assert_eq!((frames[8].load, frames[9].load), (-1., -0.9));
        // 451580 → 4511E0: the clunk once, on the first frame past 0.13 s.
        assert!(!up.clunk);
        assert_eq!(frames.iter().position(|f| f.clunk), Some(8));
        assert_eq!(frames.iter().filter(|f| f.clunk).count(), 1);
        // The dip recovers by 0.003 per frame.
        assert!((frames[0].scale - up.scale - 0.003).abs() < 1e-6 || frames[0].scale == 1.);
        // No wobble before 0.18 s, then 1.75 · 5 Hz at 0.15 · (1 − 2/6).
        assert!(frames[..10].iter().all(|f| f.pitch == 1.));
        let peak = frames.iter().map(|f| (f.pitch - 1.).abs()).fold(0., f32::max);
        assert!(peak > 0.05 && peak <= 0.1 + 1e-6, "{peak}");
        // Done after 0.18 + 0.25 s.
        let late: Vec<_> = (0..20).map(|_| shift.frame(2, dt)).collect();
        assert!(late.iter().all(|f| f.pitch == 1.));
    }

    #[test]
    fn a_downshift_wobbles_at_once_and_reverse_is_silent() {
        let mut shift = Shift::new(lacetti(), 7);
        for gear in [1, 2, 3] {
            for _ in 0..60 {
                shift.frame(gear, 0.016);
            }
        }
        let down = shift.frame(2, 0.016);
        assert!((0..60).all(|_| !shift.frame(2, 0.016).clunk), "no clunk after a downshift");
        let mut shift = Shift::new(lacetti(), 7);
        for gear in [1, 2, 3] {
            for _ in 0..60 {
                shift.frame(gear, 0.016);
            }
        }
        assert_eq!(shift.frame(2, 0.016), down);
        assert_eq!(down.scale, 1.);
        assert!(down.load > 0.);
        // 15 Hz, loudness 0.15 · (1 − 2/6) = 0.1, from the first frame.
        assert_eq!(down.pitch, 1.);
        let next = shift.frame(2, 0.016);
        let expected = ((15. * 0.016 * std::f32::consts::TAU) as f64).sin() as f32 * 1.75 * (1. - 0.016 / 0.3) * 0.1;
        assert!((next.pitch - 1. - expected).abs() < 1e-5, "{next:?} {expected}");
        // First to reverse and back counts as no shift.
        let mut shift = Shift::new(lacetti(), 7);
        shift.frame(FIRST, 0.016);
        for _ in 0..60 {
            shift.frame(FIRST, 0.016);
        }
        shift.frame(REVERSE, 0.016);
        assert!(!shift.up && shift.elapsed > 0.9 && shift.loudness == 0.);
    }

    impl Effects {
        fn with_load(self, load: f32) -> Self {
            Self { load, ..self }
        }
    }
}
