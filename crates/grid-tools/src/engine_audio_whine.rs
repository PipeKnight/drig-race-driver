//! The gear whine of 4518E0 (docs/AUDIO_RESEARCH.md §24). Every car's
//! gearbox plays the GearPreset sample of its gear (`01_gear`…, from the car's
//! mec bank) at the GearBox emitter: the pitch rises from the entry's
//! basePitch to its maxPitch between the gear's shift speeds, the gear.ini
//! `[Envelope]` modulates level and pitch with the road speed, a lift fades
//! it, and an upshift adds the engine.ini `fUpshiftWhine*` wobble. The
//! object is car-audio +0x84C; its parameter block [+0xC] is car-audio
//! +0x10B0, which holds the envelope (423600) and the `[GearWhine]` keys
//! (42F880, +0x510…).
use crate::{car::Car, collision_audio::read_infos, tyre_audio::lin2db};
use grid_formats::audio_car_setup;
use std::path::Path;

const ENVELOPE: usize = 128;
/// 416820: m/s to mph.
const MPH: f32 = 2.236_936_3;

/// One car's whine parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The car's GearPreset entries (car record +0x20): basePitch, maxPitch.
    pub gears: Vec<[f32; 2]>,
    /// `[Envelope]` Length (+0x100), Volume (+0x104) and Pitch (+0x304).
    length: f32,
    volume: [f32; ENVELOPE],
    pitch: [f32; ENVELOPE],
    /// `[GearWhine]` MPHForNormalEnvelopeRate (+0x510), MaxLowLoadEffectMultiple
    /// (+0x514), EnvelopeFullVolumeSpeed (+0x538), EnvelopeFadedOutSpeed
    /// (+0x53C).
    envelope_mph: f32,
    low_load: f32,
    full_volume_mph: f32,
    faded_out_mph: f32,
    /// engine.ini `[Physics]` fUpshiftWhineLoudness (+0x524), Rate (+0x528),
    /// Length (+0x52C), Randomness (+0x530) and StartDelay (+0x534).
    loudness: f32,
    rate: f32,
    wobble_length: f32,
    randomness: f32,
    start_delay: f32,
    /// The Gearwhine CollisionInfo's minVel (+0x54) and minVol (+0x5C, a
    /// linear factor here; maxVol 0.6 is never read).
    min_speed: f32,
    factor: f32,
    /// carsetup VolumeTrims `gear` (car record +0xC8), dB.
    trim: f32,
}

fn section<'a>(ini: &'a str, name: &str) -> impl Iterator<Item = (&'a str, &'a str)> {
    let mut current = "";
    let name = name.to_owned();
    ini.lines().map(str::trim).filter_map(move |line| {
        if line.starts_with('[') && line.ends_with(']') {
            current = &line[1..line.len() - 1];
            None
        } else if current == name {
            line.split_once('=').map(|(k, v)| (k.trim(), v.trim()))
        } else {
            None
        }
    })
}
fn key<'a>(ini: &'a str, part: &str, name: &str) -> Result<Option<&'a str>, String> {
    let mut found = section(ini, part).filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v);
    let value = found.next();
    if found.next().is_some() {
        return Err(format!("duplicate [{part}] {name}"));
    }
    Ok(value)
}
fn number(ini: &str, part: &str, name: &str) -> Result<f32, String> {
    let text = key(ini, part, name)?.ok_or_else(|| format!("missing [{part}] {name}"))?;
    let value: f32 = text.parse().map_err(|_| format!("invalid [{part}] {name}"))?;
    if !value.is_finite() || value.abs() > 1e6 {
        return Err(format!("[{part}] {name} out of range"));
    }
    Ok(value)
}
fn table(ini: &str, name: &str) -> Result<[f32; ENVELOPE], String> {
    let text = key(ini, "Envelope", name)?.ok_or_else(|| format!("missing [Envelope] {name}"))?;
    let values: Vec<f32> = text.split(',').map(|v| v.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|_| format!("invalid [Envelope] {name}"))?;
    if values.iter().any(|v| !v.is_finite()) {
        return Err(format!("[Envelope] {name} out of range"));
    }
    values.try_into().map_err(|_| format!("[Envelope] {name} is not {ENVELOPE} values"))
}

impl Settings {
    /// From the car's carsetup row, its gear.ini and engine.ini, and the
    /// Gearwhine CollisionInfo.
    pub fn load(root: &Path, car: &Car) -> Result<Self, String> {
        let read = |path: &str| std::fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"));
        let setup = read("audio/carsetup.bin")?;
        let setup = audio_car_setup::read(&setup, car.stem()).map_err(|e| format!("carsetup: {e:?}"))?;
        let gear = read(setup.gear_ini)?;
        let engine = read(setup.engine_ini)?;
        let infos = read_infos(&read("audio/CollisionInfos.bin")?)?;
        let info = infos.get(crate::mech_audio::WHINE).ok_or("CollisionInfo Gearwhine absent")?;
        Self::parse(
            &setup,
            std::str::from_utf8(&gear).map_err(|e| e.to_string())?,
            std::str::from_utf8(&engine).map_err(|e| e.to_string())?,
            [info.min_vel, info.min_vol],
        )
    }

    pub fn parse(setup: &audio_car_setup::Setup, gear: &str, engine: &str, info: [f32; 2]) -> Result<Self, String> {
        // 4377F0 plays entry k of the preset; the port's mec bank holds the
        // samples in gear order (`mech_audio::WHINE`).
        for (k, g) in setup.gears.iter().enumerate() {
            if g.sample != format!("{:02}_gear", k + 1) {
                return Err(format!("GearPreset entry {} plays {}", k + 1, g.sample));
            }
        }
        // 42F770: engine.ini GearWhine = N; N ≥ 2 reads the upshift wobble
        // (else its defaults), N ≥ 3 also the 0 mph frequencies (unused).
        let level = key(engine, "Physics", "GearWhine")?.map_or(Ok(0), |v| v.parse::<u32>().map_err(|_| "invalid GearWhine".to_owned()))?;
        let upshift = |name: &str, default: f32| if level >= 2 { number(engine, "Physics", name) } else { Ok(default) };
        Ok(Self {
            gears: setup.gears.iter().map(|g| [g.base_pitch, g.max_pitch]).collect(),
            length: number(gear, "Envelope", "Length")?,
            volume: table(gear, "Volume")?,
            pitch: table(gear, "Pitch")?,
            envelope_mph: number(gear, "GearWhine", "MPHForNormalEnvelopeRate")?,
            low_load: number(gear, "GearWhine", "MaxLowLoadEffectMultiple")?,
            full_volume_mph: number(gear, "GearWhine", "EnvelopeFullVolumeSpeed")?,
            faded_out_mph: number(gear, "GearWhine", "EnvelopeFadedOutSpeed")?,
            wobble_length: upshift("fUpshiftWhineLength", 0.25)?,
            loudness: upshift("fUpshiftWhineLoudness", 0.)?,
            randomness: upshift("fUpshiftWhineRandomness", 10.)?,
            start_delay: upshift("fUpshiftWhineStartDelay", 0.2)?,
            rate: upshift("fUpshiftWhineRate", 4.)?,
            min_speed: info[0],
            factor: info[1],
            trim: setup.trims[0],
        })
    }
}

/// What 4518E0 reads from the vehicle each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// +0x196C: 0 neutral, 1.. forward, 9 reverse.
    pub gear: u32,
    /// +0x1974: seconds since the last gear change.
    pub shift_time: f32,
    /// The driven wheels (+0x18D8, count +0x18F0): mean of wheel +0xE0 ·
    /// +0x104 (990FB0 · 990DB0), m/s.
    pub wheel_speed: f32,
    /// How many of them have no contact (wheel +0xC0 = 0, 990FD0).
    pub airborne: u32,
    /// +0x1FA0 (fuel) is not 0.
    pub fuel: bool,
    /// +0x185C: the throttle the drivetrain uses.
    pub throttle: f32,
    /// The gear's shift speeds (44BAD0): up +0x191C[g] (above 90 m/s, the
    /// top gear, the previous gear's scaled by the ratios +0x18F4) and down
    /// +0x1944[g], m/s.
    pub top: f32,
    pub bottom: f32,
}

impl Input {
    pub fn vehicle(words: &[u32]) -> Self {
        let f = |o: usize| words.get(o / 4).copied().map_or(0., f32::from_bits);
        let w = |o: usize| words.get(o / 4).copied().unwrap_or(0);
        let gear = w(0x196c);
        let driven = (w(0x18f0) as usize).min(4);
        let mut sum = 0.;
        let mut airborne = 0;
        for slot in 0..driven {
            let wheel = w(0x18d8 + slot * 4) as usize;
            if w(wheel + 0xc0) == 0 {
                airborne += 1;
            }
            sum += f(wheel + 0xe0) * f(wheel + 0x104);
        }
        let g = if gear == 9 { 1 } else { gear as usize };
        let mut top = f(0x191c + g * 4);
        if g > 0 && top > 90. && f(0x18f0 + g * 4) > 0. {
            top = (f(0x18f4 + g * 4) / f(0x18f0 + g * 4)) * f(0x1918 + g * 4);
        }
        Self {
            gear,
            shift_time: f(0x1974),
            wheel_speed: if driven > 0 { (1. / driven as f32) * sum } else { 0. },
            airborne,
            fuel: f(0x1fa0) != 0.,
            throttle: f(0x185c),
            top,
            bottom: f(0x1944 + g * 4),
        }
    }
}

/// What the whine plays this frame: GearPreset `entry` at `db` (voice level
/// before group and distance) and `pitch`. `start` counts 437A20's and
/// 4377F0's (re)starts of the sample (one per change of gear) modulo 16, so
/// a player restarts it once per change even when a frame repeats or drops.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Voice {
    pub entry: usize,
    pub start: u8,
    pub db: f32,
    pub pitch: f32,
}

impl Voice {
    /// One mailbox word: db bits, pitch × 10⁴, entry + 1 (0: none), start.
    pub fn pack(voice: Option<Self>) -> u64 {
        voice.map_or(0, |v| {
            u64::from(v.db.to_bits())
                | ((v.pitch.clamp(0., 6.) * 10_000.).round() as u64) << 32
                | ((v.entry as u64).min(14) + 1) << 48
                | u64::from(v.start & 15) << 52
        })
    }
    pub fn unpack(word: u64) -> Option<Self> {
        let entry = (word >> 48) & 15;
        (entry != 0).then(|| Self {
            entry: entry as usize - 1,
            start: ((word >> 52) & 15) as u8,
            db: f32::from_bits(word as u32),
            pitch: ((word >> 32) & 0xffff) as f32 / 10_000.,
        })
    }
}

/// The 4518E0 object (+0x10…+0x54) and its block's state (+0x504…+0x544).
#[derive(Clone, Debug)]
pub struct Whine {
    settings: Settings,
    /// +0x10: 437A20 ran.
    started: bool,
    /// +0x1C: smoothed wheel speed, m/s; +0x20: smoothed mph, 0…200; +0x24:
    /// smoothed mean of the driven wheels.
    speed: f32,
    mph: f32,
    mean: f32,
    /// Block +0x50C: envelope position, s.
    phase: f32,
    /// Block +0x504: the gear the sample was started for; +0x508: it was an
    /// upshift.
    gear: u32,
    up: bool,
    /// Block +0x520: the wobble's random term, 0…1.
    random: f32,
    /// Block +0x540: seconds below half throttle; +0x544: the lift's delay.
    lifted: f32,
    delay: f32,
    seed: u32,
    starts: u8,
}

impl Whine {
    pub fn new(settings: Settings, seed: u32) -> Self {
        Self {
            settings,
            started: false,
            speed: 0.,
            mph: 0.,
            mean: 0.,
            phase: 0.,
            gear: 0,
            up: false,
            random: 0.,
            lifted: 0.,
            delay: 0.,
            seed: seed | 1,
            starts: 0,
        }
    }

    /// rand() & 0xFF [I: the CRT sequence is not reproduced].
    fn byte(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x & 0xff) as f32
    }

    /// 4518E0 for `dt` seconds. `None` before the first frame's start.
    pub fn frame(&mut self, dt: f32, input: Input) -> Option<Voice> {
        let s = &self.settings;
        let starts = self.starts;
        if !self.started {
            // 437A20: start entry 0, envelope and gear state cleared.
            self.started = true;
            self.starts = self.starts.wrapping_add(1);
            self.phase = 0.;
            self.gear = 0;
            self.lifted = 0.;
        }
        // 416820.
        let mut level = 1. - 0.125 * input.airborne as f32;
        let (speed, mean) = (self.speed, self.mean);
        self.mean = (input.wheel_speed - mean) * 0.1 + mean;
        let mph = ((speed * MPH - self.mph) * 0.1 + self.mph).clamp(0., 200.);
        self.speed = (mean - speed) * 0.1 + speed;
        self.mph = if input.fuel { mph } else { 0. };
        // The envelope runs at mph / MPHForNormalEnvelopeRate (4169A0).
        self.phase += (self.mph / s.envelope_mph) * dt;
        if s.length <= self.phase {
            self.phase -= s.length;
        }
        let at = self.phase / s.length;
        let at = (at - (at as i32) as f32) * ENVELOPE as f32;
        let i = (at as i32).clamp(0, ENVELOPE as i32 - 1) as usize;
        let fraction = at - (at as i32) as f32;
        let next = (i + 1) % ENVELOPE;
        let envelope_volume = s.volume[i] * (1. - fraction) + s.volume[next] * fraction;
        let envelope_pitch = s.pitch[i] * (1. - fraction) + s.pitch[next] * fraction;
        // 4378A0: the sample follows the gear; an upshift wobbles the pitch.
        let mut wobble = 1.;
        let gear = input.gear;
        if gear == 0 {
            level = 0.;
        } else {
            if gear != self.gear {
                self.up = self.gear < gear && gear != 9;
                self.gear = gear;
                // 4377F0 in the same frame as 437A20 replaces its start.
                if starts == self.starts {
                    self.starts = self.starts.wrapping_add(1);
                }
            }
            let s = &self.settings;
            let t = input.shift_time;
            if t <= s.start_delay || !self.up {
                self.random = self.byte() * 0.003_906_25;
            } else {
                let t = t - s.start_delay;
                let length = s.wobble_length * gear as f32;
                if t < length {
                    let sine = (f64::from(s.rate * t * 6.283_185_5)).sin() as f32;
                    wobble += sine * ((self.random * 2. - 1.) * s.randomness * 0.01 + 1.) * (1. - t / length) * s.loudness;
                }
            }
        }
        // 4166E0: below half throttle, after a random 0.05–0.5 s, the whine
        // fades towards 1 − MaxLowLoadEffectMultiple at low speed.
        let mut throttle = input.throttle;
        if throttle < 0. {
            throttle = 0.;
            self.lifted += dt;
        } else if throttle < 0.5 {
            self.lifted += dt;
        } else if self.lifted > 0. {
            self.delay = self.byte() * 0.001_764_705_9 + 0.05;
            self.lifted = 0.;
        }
        let s = &self.settings;
        let off = ((self.lifted - self.delay) * 100.).clamp(0., 1.);
        let slow = ((s.faded_out_mph - self.mph) / (s.faded_out_mph - s.full_volume_mph)).clamp(0., 1.);
        let load = (1. - slow * off * (1. - throttle)) * s.low_load + (1. - s.low_load);
        if self.mph < 10. {
            level *= self.mph * 0.1;
        }
        // 44BAD0. Neutral keeps the last gear's sample at −48 dB.
        let entry = match self.gear {
            0 | 9 => 0,
            g => g as usize - 1,
        };
        if gear == 0 {
            return Some(Voice { entry: entry.min(s.gears.len().saturating_sub(1)), start: self.starts & 15, db: -48., pitch: 1. });
        }
        let [base, max] = *s.gears.get(entry).or(s.gears.last())?;
        let t = (self.speed - input.bottom) / (input.top - input.bottom);
        let pitch = (max - base) * t + base;
        let mut volume = envelope_volume * 0.5 + 0.5;
        if load >= 1. {
            volume = 1.;
        } else if load > 0. {
            volume = if volume < 1. { (1. - volume) * load + volume } else { volume - (volume - 1.) * load };
        }
        if t > 1. {
            volume -= t - 1.;
        }
        let shimmer = envelope_pitch * 0.1 + 0.9;
        let mut shimmer_load = 1.;
        if load < 1. {
            shimmer_load = shimmer;
            if load > 0. {
                shimmer_load = if shimmer < 1. { (1. - shimmer) * load + shimmer } else { shimmer - (shimmer - 1.) * load };
            }
        }
        // 413130 over the Gearwhine CollisionInfo: ramps in up to minVel.
        let factor = if s.min_speed < self.speed {
            s.factor
        } else {
            let x = self.speed / s.min_speed;
            if x >= 1. { s.factor } else if x <= 0. { 0. } else { s.factor * x }
        };
        Some(Voice {
            entry: entry.min(s.gears.len() - 1),
            start: self.starts & 15,
            db: (input.throttle - 1.) + lin2db(level * volume * factor) + s.trim,
            pitch: wobble * shimmer_load * pitch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            gears: vec![[0.75, 1.12], [0.85, 1.023], [0.95, 1.023]],
            length: 7.,
            volume: [0.5; ENVELOPE],
            pitch: [1.; ENVELOPE],
            envelope_mph: 40.,
            low_load: 1.,
            full_volume_mph: 40.,
            faded_out_mph: 80.,
            loudness: 0.075,
            rate: 10.,
            wobble_length: 0.01,
            randomness: 75.,
            start_delay: 0.01,
            min_speed: 5.,
            factor: 1.,
            trim: -1.,
        }
    }

    #[test]
    fn the_pitch_rises_between_the_shift_speeds_and_a_lift_fades_it() {
        let mut w = Whine::new(settings(), 7);
        let input = Input { gear: 2, shift_time: 5., wheel_speed: 20., airborne: 0, fuel: true, throttle: 1., top: 30., bottom: 10. };
        let first = w.frame(0.016, input).unwrap();
        assert_eq!((first.entry, first.start), (1, 1));
        let mut v = first;
        for _ in 0..400 {
            v = w.frame(0.016, input).unwrap();
        }
        assert_eq!(v.start, 1);
        // Speed 20 m/s, halfway: 0.85 + 0.5·0.173; full volume, trim −1 dB.
        assert!((v.pitch - (0.85 + 0.5 * 0.173)).abs() < 1e-3, "{v:?}");
        assert!((v.db + 1.).abs() < 1e-3, "{v:?}");
        // Neutral silences it; a lift at low speed fades it after the delay.
        assert_eq!(w.frame(0.016, Input { gear: 0, ..input }).unwrap().db, -48.);
        let mut slow = input;
        slow.wheel_speed = 8.;
        for _ in 0..400 {
            w.frame(0.016, slow);
        }
        let mut lift = w.frame(0.016, Input { throttle: 0., ..slow }).unwrap();
        assert_eq!(lift.start, 1);
        for _ in 0..60 {
            lift = w.frame(0.016, Input { throttle: 0., ..slow }).unwrap();
        }
        // 18 mph is below EnvelopeFullVolumeSpeed: the load term falls to 0,
        // so the envelope's 0.5 gives 0.75 (−2.5 dB), and the throttle term
        // adds −1 dB.
        assert!((lift.db - (-2. + 20. * 0.75f32.log10())).abs() < 1e-3, "{lift:?}");
    }

    #[test]
    fn an_upshift_wobbles_the_pitch_after_its_start_delay() {
        let mut w = Whine::new(settings(), 7);
        let input = Input { gear: 1, shift_time: 5., wheel_speed: 10., airborne: 0, fuel: true, throttle: 1., top: 30., bottom: 0. };
        for _ in 0..300 {
            w.frame(0.016, input);
        }
        let steady = w.frame(0.016, input).unwrap().pitch;
        assert!((steady - (0.75 + 0.37 / 3.)).abs() < 1e-4, "{steady}");
        let second = 0.85 + 0.173 / 3.;
        let shifted = w.frame(0.016, Input { gear: 2, shift_time: 0.015, ..input }).unwrap();
        assert_eq!((shifted.entry, shifted.start), (1, 2));
        assert!((shifted.pitch / second - 1.).abs() > 1e-3, "{shifted:?}");
        let late = w.frame(0.016, Input { gear: 2, shift_time: 1., ..input }).unwrap();
        assert_eq!(late.start, 2);
        assert!((late.pitch - second).abs() < 1e-4, "{late:?}");
    }

    /// The Lacetti's parameters: preset holden, the shared gear.ini and
    /// the engine.ini wobble, the Gearwhine CollisionInfo.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn the_lacetti_whine_settings_load() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let s = Settings::load(&root, &Car::default()).unwrap();
        assert_eq!(s.gears.len(), 6);
        assert_eq!(s.gears[0], [0.75, 1.12]);
        assert_eq!(s.gears[5], [1.0, 1.2]);
        assert_eq!((s.length, s.volume[0], s.pitch[0]), (7., 0.621429, 1.010714));
        assert_eq!((s.envelope_mph, s.low_load, s.full_volume_mph, s.faded_out_mph), (40., 1., 40., 80.));
        assert_eq!((s.wobble_length, s.loudness, s.randomness, s.start_delay, s.rate), (0.01, 0.075, 75., 0.01, 10.));
        assert_eq!((s.min_speed, s.factor, s.trim), (5., 1., -1.));
    }

    #[test]
    fn the_mailbox_word_round_trips() {
        let v = Voice { entry: 3, start: 9, db: -7.25, pitch: 1.0625 };
        assert_eq!(Voice::unpack(Voice::pack(Some(v))), Some(v));
        assert_eq!(Voice::unpack(Voice::pack(None)), None);
    }
}
