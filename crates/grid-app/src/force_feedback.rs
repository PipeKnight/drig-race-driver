//! Force feedback and pad rumble: the original's GameFFB (docs/FORCE_FEEDBACK.md).
//!
//! The pure part, tested against the decompiled formulas: the `forcefeedback/*.xml` setup, the
//! snapshot `7E9D60` read from the vehicle record, the FFB task `7ECAC0` with its effects and
//! vibration samples, and the DirectInput device's software damping (`B8CD80`) and XInput motor
//! scaling. The device output is `force_feedback/windows.rs` (wheel and XInput pad) and
//! `force_feedback/android.rs` (pad or phone vibrator); the Bevy glue is `ffb.rs`.

use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(target_os = "android")]
pub mod android;
#[cfg(windows)]
pub mod windows;

// ------------------------------------------------------------------------------------- setup XML

/// One element's name and attributes. The shipped files are flat lists of empty elements.
struct Element<'a> {
    name: &'a str,
    attributes: Vec<(&'a str, &'a str)>,
}

impl Element<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.attributes.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
    fn number(&self, key: &str, into: &mut f32) {
        if let Some(v) = self.get(key) {
            *into = number(v);
        }
    }
}

const MAXIMUM_TEXT: usize = 1 << 20;
const MAXIMUM_ELEMENTS: usize = 4096;

/// Elements of a small XML file, skipping comments, declarations and end tags.
fn elements(text: &str) -> Result<Vec<Element<'_>>, String> {
    if text.len() > MAXIMUM_TEXT {
        return Err("force feedback XML exceeds 1 MiB".into());
    }
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let skip = |rest: &str, end: &str| {
            rest.find(end)
                .map(|e| e + end.len())
                .ok_or_else(|| format!("unterminated {}", &rest[..rest.len().min(8)]))
        };
        if rest.starts_with("<!--") {
            rest = &rest[skip(rest, "-->")?..];
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("</") || rest.starts_with("<!") {
            rest = &rest[skip(rest, ">")?..];
            continue;
        }
        let body = &rest[1..];
        let end = body
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .ok_or("unterminated element")?;
        let name = &body[..end];
        let mut s = &body[end..];
        let mut attributes = Vec::new();
        loop {
            s = s.trim_start();
            if let Some(r) = s.strip_prefix("/>").or_else(|| s.strip_prefix('>')) {
                rest = r;
                break;
            }
            let eq = s.find('=').ok_or("attribute without a value")?;
            let key = s[..eq].trim();
            s = s[eq + 1..].trim_start();
            let quote = s
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or("unquoted attribute")?;
            let close = s[1..].find(quote).ok_or("unterminated attribute")?;
            attributes.push((key, &s[1..1 + close]));
            s = &s[close + 2..];
        }
        if out.len() == MAXIMUM_ELEMENTS {
            return Err("too many force feedback elements".into());
        }
        out.push(Element { name, attributes });
    }
    Ok(out)
}

/// The longest numeric prefix, as the C library reads it (`wheelScale = "0.5f"` in the shipped
/// effectsetup.xml reads 0.5); 0 without one.
fn number(v: &str) -> f32 {
    let v = v.trim();
    let end = v
        .find(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c)))
        .unwrap_or(v.len());
    (0..=end)
        .rev()
        .find_map(|i| v[..i].parse::<f32>().ok())
        .unwrap_or(0.)
}

/// FFBDevice (`7EB540`). Constructor values 1, 1, 0, 10, 1, 0.1, off.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceSetup {
    pub name: String,
    pub scale_force: f32,
    pub scale_friction: f32,
    pub base_friction: f32,
    pub max_friction: f32,
    pub scale_effects: f32,
    pub delay: f32,
    pub native_friction: bool,
}

/// FFBCar: gains for one car folder name (default 1 [I]).
#[derive(Clone, Debug, PartialEq)]
pub struct CarSetup {
    pub name: String,
    pub scale_force: f32,
    pub scale_friction: f32,
    pub scale_effects: f32,
}

/// FFBSample (`7E7DA0`): at most 16 wave and envelope digits; a wave digit is stored as
/// `d - '5'`, an envelope digit as `e - '0'`; the counts are the last non-zero character + 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    wave: [i8; 16],
    waves: usize,
    envelope: [i8; 16],
    envelopes: usize,
    pub channel: i32,
    pub wheel_scale: f32,
    pub rumbler_scale: f32,
}

impl Sample {
    fn new(name: &str, wave: &str, envelope: &str, channel: i32, wheel: f32, rumbler: f32) -> Self {
        let digits = |text: &str, zero: u8| {
            let mut out = [0i8; 16];
            let mut count = 16;
            for (i, b) in text.bytes().take(16).enumerate() {
                if b != 0 {
                    count = i + 1;
                    out[i] = b.wrapping_sub(zero) as i8;
                }
            }
            (out, count)
        };
        let (wave, waves) = digits(wave, b'5');
        let (envelope, envelopes) = digits(envelope, b'0');
        Self {
            name: name.into(),
            wave,
            waves,
            envelope,
            envelopes,
            channel,
            wheel_scale: wheel,
            rumbler_scale: rumbler,
        }
    }
}

/// FFBCaster (`7EBCC0`); `Default` is the constructor `7EDAB0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caster {
    pub scale: f32,
    pub toe: f32,
    pub camber: f32,
    pub trail: f32,
    pub threshold_low: f32,
    pub friction_low: f32,
    pub threshold_high: f32,
    pub friction_high: f32,
    pub angle_threshold_low: f32,
    pub angle_friction_low: f32,
    pub angle_threshold_high: f32,
    pub angle_friction_high: f32,
    pub corner_point: f32,
    pub exponent: f32,
    pub deadzone_low: f32,
    pub deadzone_high: f32,
    pub preload: f32,
}

impl Default for Caster {
    fn default() -> Self {
        Self {
            scale: 0.,
            toe: 0.,
            camber: 0.001,
            trail: -0.001,
            threshold_low: 1.,
            friction_low: 0.,
            threshold_high: 2.,
            friction_high: 10.,
            angle_threshold_low: 10.,
            angle_friction_low: 10.,
            angle_threshold_high: 18.,
            angle_friction_high: 0.1,
            corner_point: 1.,
            exponent: 0.2,
            deadzone_low: 3.,
            deadzone_high: -5.,
            preload: 0.,
        }
    }
}

/// A speed band: `low`/`high` at or beyond the thresholds, linear between.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Band {
    pub threshold_low: f32,
    pub threshold_high: f32,
    pub low: f32,
    pub high: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Caster(Caster),
    /// FFBlateral: scale and the static friction band over speed.
    Lateral { scale: f32, friction: Band },
    /// FFBCollision: the sample set is drawn each time it fires.
    Collision {
        scale: f32,
        threshold_low: f32,
        threshold_high: f32,
        freq: f32,
        length: f32,
        sample: String,
    },
    /// FFBSurface: `name` is the surface's mechanics name; `freq` and `envelope` bands.
    Surface {
        scale: f32,
        name: String,
        freq: Band,
        envelope: Band,
        length: f32,
        sample: usize,
        grip: f32,
    },
    GearChange { scale: f32, gear: i32, freq: f32, length: f32, sample: usize },
    WheelDamage { scale: f32, damage: Band, sample: usize },
    /// FFBSkid: `length` is the lengthLow/High band.
    Skid { scale: f32, length: Band, freq: f32, sample: usize },
    Suspension { scale: f32, threshold_low: f32, threshold_high: f32, length: f32, freq: f32, sample: usize },
}

/// MSVC `rand()`, which picks a member of a sample set (`7EB030`). [A] seed: the CRT's 1.
#[derive(Clone, Debug, PartialEq)]
struct Rand(u32);

impl Rand {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7fff
    }
}

/// The three files of `forcefeedback/forceFeedbackPaths.xml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub devices: Vec<DeviceSetup>,
    pub cars: Vec<CarSetup>,
    pub samples: Vec<Sample>,
    /// In the parser order of `7ECA30`: caster, lateral, collision, surface, gear change, wheel
    /// damage, skid, suspension.
    pub effects: Vec<Effect>,
    rand: Rand,
}

impl Setup {
    pub fn load(root: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            let path = root.join("forcefeedback").join(name);
            let size = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if size.len() > MAXIMUM_TEXT as u64 {
                return Err(format!("{} exceeds 1 MiB", path.display()));
            }
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        Self::parse(&read("devicesetup.xml")?, &read("effectsetup.xml")?, &read("carsetup.xml")?)
    }

    pub fn parse(devices: &str, effects: &str, cars: &str) -> Result<Self, String> {
        let mut setup = Self {
            devices: Vec::new(),
            cars: Vec::new(),
            samples: Vec::new(),
            effects: Vec::new(),
            rand: Rand(1),
        };
        for e in elements(devices)?.iter().filter(|e| e.name == "FFBDevice") {
            let mut d = e
                .get("template")
                .and_then(|t| setup.devices.iter().find(|d| d.name == t))
                .cloned()
                .unwrap_or(DeviceSetup {
                    name: String::new(),
                    scale_force: 1.,
                    scale_friction: 1.,
                    base_friction: 0.,
                    max_friction: 10.,
                    scale_effects: 1.,
                    delay: 0.1,
                    native_friction: false,
                });
            d.name = e.get("name").unwrap_or("").into();
            e.number("scaleForce", &mut d.scale_force);
            e.number("scaleFriction", &mut d.scale_friction);
            e.number("baseFriction", &mut d.base_friction);
            e.number("maxFriction", &mut d.max_friction);
            e.number("scaleEffects", &mut d.scale_effects);
            e.number("delay", &mut d.delay);
            if let Some(v) = e.get("nativeFriction") {
                d.native_friction = number(v) != 0.;
            }
            setup.devices.push(d);
        }
        for e in elements(cars)?.iter().filter(|e| e.name == "FFBCar") {
            let mut c = e
                .get("template")
                .and_then(|t| setup.cars.iter().find(|c| c.name == t))
                .cloned()
                .unwrap_or(CarSetup {
                    name: String::new(),
                    scale_force: 1.,
                    scale_friction: 1.,
                    scale_effects: 1.,
                });
            c.name = e.get("name").unwrap_or("").into();
            e.number("scaleForce", &mut c.scale_force);
            e.number("scaleFriction", &mut c.scale_friction);
            e.number("scaleEffects", &mut c.scale_effects);
            setup.cars.push(c);
        }
        let all = elements(effects)?;
        let of = |name: &'static str| all.iter().filter(move |e| e.name == name);
        for e in of("FFBSample") {
            let mut wheel = 1.;
            let mut rumbler = 1.;
            e.number("wheelScale", &mut wheel);
            e.number("rubmlerScale", &mut rumbler);
            setup.samples.push(Sample::new(
                e.get("name").unwrap_or(""),
                e.get("wave").unwrap_or(""),
                e.get("envelope").unwrap_or(""),
                e.get("channel").map_or(0, |c| number(c) as i32),
                wheel,
                rumbler,
            ));
        }
        for e in of("FFBCaster") {
            let mut c = Caster::default();
            for (key, field) in [
                ("scale", &mut c.scale),
                ("toe", &mut c.toe),
                ("trail", &mut c.trail),
                ("camber", &mut c.camber),
                ("thresholdLow", &mut c.threshold_low),
                ("frictionLow", &mut c.friction_low),
                ("thresholdHigh", &mut c.threshold_high),
                ("frictionHigh", &mut c.friction_high),
                ("angleThresholdLow", &mut c.angle_threshold_low),
                ("angleFrictionLow", &mut c.angle_friction_low),
                ("angleThresholdHigh", &mut c.angle_threshold_high),
                ("angleFrictionHigh", &mut c.angle_friction_high),
                ("cornerPoint", &mut c.corner_point),
                ("exponent", &mut c.exponent),
                ("preload", &mut c.preload),
                ("deadzoneLow", &mut c.deadzone_low),
                ("deadzoneHigh", &mut c.deadzone_high),
            ] {
                e.number(key, field);
            }
            setup.effects.push(Effect::Caster(c));
        }
        let value = |e: &Element, key: &str| {
            let mut v = 0.;
            e.number(key, &mut v);
            v
        };
        let band = |e: &Element, low: &str, high: &str| Band {
            threshold_low: value(e, "thresholdLow"),
            threshold_high: value(e, "thresholdHigh"),
            low: value(e, low),
            high: value(e, high),
        };
        for e in of("FFBlateral") {
            setup.effects.push(Effect::Lateral {
                scale: value(e, "scale"),
                friction: band(e, "frictionLow", "frictionHigh"),
            });
        }
        for e in of("FFBCollision") {
            setup.effects.push(Effect::Collision {
                scale: value(e, "scale"),
                threshold_low: value(e, "thresholdLow"),
                threshold_high: value(e, "thresholdHigh"),
                freq: value(e, "freq"),
                length: value(e, "length"),
                sample: e.get("sample").unwrap_or("").into(),
            });
        }
        // The other effects draw their sample at load and are dropped without one.
        for e in of("FFBSurface") {
            let Some(sample) = setup.pick(e.get("sample").unwrap_or("")) else { continue };
            setup.effects.push(Effect::Surface {
                scale: value(e, "scale"),
                name: e.get("name").unwrap_or("").into(),
                freq: band(e, "freqLow", "freqHigh"),
                envelope: band(e, "scaleLow", "scaleHigh"),
                length: value(e, "length"),
                sample,
                grip: value(e, "grip"),
            });
        }
        for e in of("FFBGearChange") {
            let Some(sample) = setup.pick(e.get("sample").unwrap_or("")) else { continue };
            setup.effects.push(Effect::GearChange {
                scale: value(e, "scale"),
                gear: e.get("gear").map_or(0, |g| number(g) as i32),
                freq: value(e, "freq"),
                length: value(e, "length"),
                sample,
            });
        }
        for e in of("FFBWheelDamage") {
            let Some(sample) = setup.pick(e.get("sample").unwrap_or("")) else { continue };
            setup.effects.push(Effect::WheelDamage {
                scale: value(e, "scale"),
                damage: band(e, "scaleLow", "scaleHigh"),
                sample,
            });
        }
        for e in of("FFBSkid") {
            let Some(sample) = setup.pick(e.get("sample").unwrap_or("")) else { continue };
            setup.effects.push(Effect::Skid {
                scale: value(e, "scale"),
                length: band(e, "lengthLow", "lengthHigh"),
                freq: value(e, "freq"),
                sample,
            });
        }
        for e in of("FFBSuspension") {
            let Some(sample) = setup.pick(e.get("sample").unwrap_or("")) else { continue };
            setup.effects.push(Effect::Suspension {
                scale: value(e, "scale"),
                threshold_low: value(e, "thresholdLow"),
                threshold_high: value(e, "thresholdHigh"),
                length: value(e, "length"),
                freq: value(e, "freq"),
                sample,
            });
        }
        Ok(setup)
    }

    /// `7EB030`: one member of the named set, `rand() % count` when it has several.
    fn pick(&mut self, name: &str) -> Option<usize> {
        let set: Vec<usize> = (0..self.samples.len()).filter(|&i| self.samples[i].name == name).collect();
        match set.len() {
            0 => None,
            1 => Some(set[0]),
            n => Some(set[self.rand.next() as usize % n]),
        }
    }

    /// `7EAA40`: the device's setup by exact name, else `default`.
    pub fn device(&self, name: &str) -> Option<&DeviceSetup> {
        let find = |n: &str| self.devices.iter().find(|d| d.name == n);
        find(name).or_else(|| find("default"))
    }

    /// The surface effects' names, for [`Snapshot::build`]'s surface ids.
    pub fn surface(&self, mechanics: &str) -> Option<u8> {
        let mut names: Vec<&str> = Vec::new();
        for e in &self.effects {
            if let Effect::Surface { name, .. } = e
                && !names.contains(&name.as_str())
            {
                names.push(name);
            }
        }
        names.iter().position(|n| *n == mechanics).map(|i| i as u8)
    }
}

// ------------------------------------------------------------------------------------- snapshot

fn word(b: &[u32], o: usize) -> f32 {
    b.get(o / 4).copied().map_or(0., f32::from_bits)
}
fn quad(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| word(b, o + i * 4))
}
fn byte(b: &[u32], o: usize) -> u8 {
    b.get(o / 4).map_or(0, |w| (w >> ((o % 4) * 8)) as u8)
}

/// The SSE length the original uses: RSQRTPS with one Newton step, 0 for 0.
fn length(squared: f32) -> f32 {
    if squared == 0. {
        return 0.;
    }
    let r = grid_world::sse_approx::rsqrt(squared);
    let y = r * squared;
    (1. - y * r) * y * 0.5 + y
}

/// One front wheel of a snapshot (0x70 bytes at +0x50 / +0xC0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelSnapshot {
    /// +0x00: the tyre's lateral axis in world space.
    pub lateral_axis: [f32; 4],
    /// +0x30..+0x38: lateral force, load and longitudinal force over m·g/4.
    pub lateral: f32,
    pub load: f32,
    pub longitudinal: f32,
    /// +0x40: contact point velocity less the wheel's rolling speed along its forward axis.
    pub slip: [f32; 4],
    /// +0x50: the surface effect id of the wheel's material.
    pub surface: Option<u8>,
    /// +0x54: the load, 0 below 0.1.
    pub ground: f32,
    /// +0x58: suspension speed (wheel +3C).
    pub suspension: f32,
    /// +0x60: damage, 1 with a tyre flag.
    pub damage: f32,
    /// +0x64: revolutions per second.
    pub revs: f32,
    /// +0x6C: detached.
    pub detached: bool,
}

/// The physical body: pose columns (x, y, z axes, position; `A69630`), linear (`A69800`) and
/// angular velocity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub pose: [f32; 16],
    pub velocity: [f32; 4],
    pub angular: [f32; 4],
}

/// `7E9D60`: what the FFB task needs of the car, one physics step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub car: String,
    pub velocity: [f32; 4],
    pub mass: f32,
    pub gear: u32,
    /// +0x1BC: the front wheel angle (+E30·+E54).
    pub steer: f32,
    /// +0x1C4: the front-axle heading (`9AE5A0`).
    pub heading: f32,
    /// +0x1C8, +0x1CC: the centre term and its limit.
    pub centre: f32,
    pub centre_limit: f32,
    pub wheels: [WheelSnapshot; 2],
    /// +0x1D8: Δv of this step's contact impulse.
    pub collision: Option<f32>,
    /// +0x1DC: the new gear when it changed this step (set by the queue).
    pub gear_event: Option<u32>,
    time: f32,
}

impl Snapshot {
    /// From the vehicle record `b` (the original layout), the physical body, the car folder
    /// name, the body's contact impulse this step (`A69930`, `None` without a contact) and a
    /// surface id lookup of a wheel's material token.
    pub fn build(
        b: &[u32],
        body: &Body,
        car: &str,
        impulse: Option<[f32; 3]>,
        surface: impl Fn(u32) -> Option<u8>,
    ) -> Self {
        let mass = word(b, 0x15a0);
        let q = 1. / (mass * 2.4525);
        let p = &body.pose;
        let rows: [[f32; 4]; 4] = std::array::from_fn(|r| std::array::from_fn(|j| p[4 * r + j]));
        let rotate = |x: f32, y: f32, z: f32| -> [f32; 4] {
            std::array::from_fn(|j| y * rows[1][j] + x * rows[0][j] + z * rows[2][j])
        };
        let (linear, angular) = (body.velocity, body.angular);
        let mut wheels = [WheelSnapshot::default(); 2];
        for (k, out) in wheels.iter_mut().enumerate() {
            let i = k + 2;
            let w = 0x1990 + 0x150 * i;
            let (a, c) = (word(b, w + 0x20), word(b, w + 0x24));
            out.lateral_axis = rotate(a, 0., -c);
            let forward = rotate(c, 0., a);
            // A697C0: the velocity of the wheel's anchor point [I: about the body's position].
            let p = quad(b, w);
            let world: [f32; 4] = std::array::from_fn(|j| {
                rows[3][j] * 1. + p[1] * rows[1][j] + p[2] * rows[2][j] + p[0] * rows[0][j]
            });
            let r: [f32; 4] = std::array::from_fn(|j| world[j] - rows[3][j]);
            let point = [
                linear[0] + (angular[1] * r[2] - angular[2] * r[1]),
                linear[1] + (angular[2] * r[0] - angular[0] * r[2]),
                linear[2] + (angular[0] * r[1] - angular[1] * r[0]),
                linear[3],
            ];
            let rim = word(b, w + 0xe0) * word(b, w + 0x104);
            out.slip = std::array::from_fn(|j| point[j] - rim * forward[j]);
            out.lateral = word(b, w + 0xb0) * q;
            out.load = word(b, w + 0x138) * q;
            out.longitudinal = word(b, w + 0xb4) * q;
            out.surface = match b.get((w + 0xc0) / 4) {
                Some(&t) if t != 0 => surface(t),
                _ => None,
            };
            out.detached = byte(b, 0x22e4 + i) != 0;
            out.damage = if byte(b, 0x22e0 + i) != 0 { 1. } else { word(b, 0xce08 + 4 * i) };
            out.ground = if out.load < 0.1 { 0. } else { out.load };
            out.suspension = word(b, w + 0x3c);
            out.revs = word(b, w + 0x104) * 0.159_154_94;
        }
        let lock = word(b, 0xe54);
        let heading = grid_tools::source_vehicle::assists::front_heading(b);
        let velocity = linear;
        let collision = impulse.and_then(|i| {
            let dv = length(i[2] * i[2] + (i[1] * i[1] + i[0] * i[0])) / mass;
            let speed = (velocity[3] * velocity[3] + velocity[2] * velocity[2])
                + (velocity[1] * velocity[1] + velocity[0] * velocity[0]);
            (dv > 0.5 || speed > 4.).then_some(dv)
        });
        Self {
            car: car.into(),
            velocity,
            mass,
            gear: b.get(0x196c / 4).copied().unwrap_or(0),
            steer: -(f64::from(lock) * f64::from(-word(b, 0xe30))) as f32,
            heading,
            centre: (f64::from(heading) / f64::from(lock)) as f32,
            centre_limit: 1.,
            wheels,
            collision,
            gear_event: None,
            time: 0.,
        }
    }

    /// |velocity| of the three spatial components.
    fn speed(&self) -> f32 {
        let v = self.velocity;
        length(v[2] * v[2] + (v[1] * v[1] + v[0] * v[0]))
    }
}

// ------------------------------------------------------------------------------------- the task

/// A sample in the queue or playing: `{sample, amplitude, freq, length, start}`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    sample: Option<usize>,
    amplitude: f32,
    freq: f32,
    length: f32,
    start: f32,
}

/// The accumulator block of one snapshot (`7ECAC0`).
#[derive(Clone, Debug, Default)]
struct Block {
    friction: f32,
    force: [f32; 2],
    camber: [f32; 2],
    grip: [f32; 2],
    gains: [f32; 3],
    centre: f32,
    one_shots: Vec<Playing>,
    loops: Vec<Playing>,
}

impl Block {
    fn add(list: &mut Vec<Playing>, sample: Option<usize>, amplitude: f32, freq: f32, length: f32) {
        if list.len() < 10 {
            list.push(Playing { sample, amplitude, freq, length, start: 0. });
        }
    }
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
fn clamp_to(x: f32, low: f32, high: f32) -> f32 {
    // The original's compare order: below `low` (or NaN) gives `low`.
    if !(low <= x) {
        low
    } else if high < x {
        high
    } else {
        x
    }
}

/// `lerp(low, high)` over `v` in the band; the endpoints at or beyond the thresholds.
fn band(b: &Band, v: f32) -> f32 {
    if v <= b.threshold_low {
        b.low
    } else if b.threshold_high <= v {
        b.high
    } else {
        let t = (v - b.threshold_low) / (b.threshold_high - b.threshold_low);
        (1. - t) * b.low + b.high * t
    }
}

/// The device's outputs (vtable 0xD43024 fields): force +0x24, friction +0x2C, the three
/// effect channels +0x30.., the centre term +0x40.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Output {
    pub force: f32,
    pub friction: f32,
    pub channels: [f32; 3],
    pub native_friction: bool,
    pub centre: f32,
}

/// GameFFB with one device: the player's wheel or pad.
#[derive(Clone, Debug)]
pub struct Ffb {
    setup: Setup,
    /// The device's name for its setup, and whether it is a wheel (`B8CD40`).
    device: String,
    wheel: bool,
    /// feedback_force, feedback_friction, feedback_effects as 0..1 (+0x94..+0x9C).
    pub gains: [f32; 3],
    /// +0x118, +0x11C: the steering range ratio and the wheelLinearity response.
    pub centre_ratio: f32,
    pub response: f32,
    enabled: bool,
    paused: bool,
    clock: f64,
    pending: Vec<Snapshot>,
    previous: Option<Snapshot>,
    one_shots: Vec<Playing>,
    loops: Vec<Playing>,
    output: Output,
}

const MAXIMUM_PENDING: usize = 64;
const MAXIMUM_PLAYING: usize = 256;

impl Ffb {
    pub fn new(setup: Setup, device: &str, wheel: bool) -> Self {
        Self {
            setup,
            device: device.into(),
            wheel,
            gains: [0.5; 3],
            centre_ratio: 0.,
            response: 0.4,
            enabled: true,
            paused: false,
            clock: 0.,
            pending: Vec::new(),
            previous: None,
            one_shots: Vec::new(),
            loops: Vec::new(),
            output: Output::default(),
        }
    }

    /// `B8CD40`: a wheel (constant-force effect) rather than a rumble device.
    #[cfg_attr(not(windows), allow(dead_code))] // The DirectInput wheel only.
    pub fn wheel(&self) -> bool {
        self.wheel
    }

    /// `7EB110`: disabling zeroes the outputs. The port reads `force_feedback` when the race
    /// starts; nothing toggles it mid-race yet.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_enabled(&mut self, on: bool) {
        if self.enabled != on {
            self.enabled = on;
            if !on {
                self.clear();
            }
        }
    }

    /// +0xA1: a paused game queues nothing and the task only advances its clock. [I] the
    /// outputs are zeroed so the wheel goes slack in the pause menu.
    pub fn set_paused(&mut self, on: bool) {
        if self.paused != on {
            self.paused = on;
            if on {
                self.clear();
            }
        }
    }

    fn clear(&mut self) {
        self.output = Output::default();
        self.pending.clear();
        self.one_shots.clear();
        self.loops.clear();
    }

    pub fn output(&self) -> Output {
        self.output
    }

    /// `7EB9C0`: queue one snapshot.
    pub fn queue(&mut self, mut s: Snapshot) {
        if !self.enabled || self.paused {
            return;
        }
        let delay = self.setup.device(&self.device).map_or(0., |d| d.delay);
        s.time = self.clock as f32 + delay;
        // 7E8340: the gear event against the previous snapshot.
        if let Some(p) = &self.previous
            && s.gear != p.gear
        {
            let (a, b) = (s.gear, p.gear);
            let count = u32::from(a == 0 || b == 0) + u32::from(a == 1 || b == 1) + u32::from(a == 9 || b == 9);
            if count < 2 {
                s.gear_event = Some(a);
            }
        }
        let k = self.response;
        let e = if k < 0. { 1. - k } else { 1. / (k + 1.) };
        let x = s.centre;
        let mut x = if x >= 0. {
            f64::from(x).powf(f64::from(e)) as f32
        } else {
            -(f64::from(-x).powf(f64::from(e)) as f32)
        };
        let r = self.centre_ratio;
        if r > 0. && r != 1. {
            x *= r;
        }
        s.centre = x;
        s.centre_limit = r;
        self.previous = Some(s.clone());
        if self.pending.len() < MAXIMUM_PENDING {
            self.pending.push(s);
        }
    }

    /// One FFB task step (`7ED380` → `7ECAC0`): advance the clock by `dt` seconds, process the
    /// due snapshots, then play the samples into the effect channels.
    pub fn process(&mut self, dt: f64) {
        self.clock += dt;
        if !self.enabled || self.paused {
            return;
        }
        let clock = self.clock;
        let due: Vec<Snapshot> = {
            let (later, now): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|s| clock < f64::from(s.time));
            self.pending = later;
            now
        };
        let now = clock as f32;
        for s in &due {
            let block = self.effects(s);
            for p in block.one_shots {
                if self.one_shots.len() < MAXIMUM_PLAYING {
                    self.one_shots.push(Playing { amplitude: p.amplitude * block.gains[2], start: now, ..p });
                }
            }
            self.loops = block
                .loops
                .into_iter()
                .map(|p| Playing { amplitude: p.amplitude * block.gains[2], start: now, ..p })
                .collect();
        }
        if !self.one_shots.is_empty() || !self.loops.is_empty() {
            self.output.channels = [0.; 3];
        }
        let mut channels = self.output.channels;
        let (setup, wheel) = (&self.setup, self.wheel);
        self.one_shots.retain(|p| play(setup, wheel, p, now, &mut channels));
        for p in &mut self.loops {
            p.start = (now / p.length).floor() * p.length;
            play(setup, wheel, p, now, &mut channels);
        }
        self.output.channels = channels;
    }

    /// Every effect, then `7EA6F0`, for one snapshot.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
    fn effects(&mut self, s: &Snapshot) -> Block {
        let mut gains = self.gains;
        if let Some(c) = self.setup.cars.iter().find(|c| c.name == s.car) {
            gains = [c.scale_force * gains[0], c.scale_friction * gains[1], c.scale_effects * gains[2]];
        }
        let mut block = Block { gains, ..Block::default() };
        let speed = s.speed();
        let [w0, w1] = &s.wheels;
        for i in 0..self.setup.effects.len() {
            match &self.setup.effects[i] {
                Effect::Caster(c) => caster(c, s, speed, &mut block),
                Effect::Lateral { scale, friction } => {
                    block.friction += band(friction, speed) * scale;
                }
                Effect::Surface { scale, name, freq, envelope, length, sample, grip } => {
                    let id = self.setup.surface(name);
                    for (k, w) in s.wheels.iter().enumerate() {
                        if w.surface.is_none() || w.surface != id || !(0. < w.ground) {
                            continue;
                        }
                        let (f, amplitude) = if envelope.threshold_low < speed {
                            (band(freq, speed), band(envelope, speed) * scale)
                        } else {
                            (freq.low, 0.)
                        };
                        Block::add(&mut block.loops, Some(*sample), amplitude, f, *length);
                        block.grip[k] = *grip;
                    }
                }
                Effect::GearChange { scale, gear, freq, length, sample } => {
                    if let Some(g) = s.gear_event
                        && (*gear == -1 || *gear as u32 == g)
                    {
                        Block::add(&mut block.one_shots, Some(*sample), *scale, *freq, *length);
                    }
                }
                Effect::WheelDamage { scale, damage, sample } => {
                    for w in [w0, w1] {
                        let d = w.damage;
                        if 0. < w.ground
                            && !w.detached
                            && damage.threshold_low <= d
                            && d <= damage.threshold_high
                            && 1. <= w.revs
                        {
                            let t = (d - damage.threshold_low) / (damage.threshold_high - damage.threshold_low);
                            let amplitude = ((1. - t) * damage.low + damage.high * t) * scale;
                            Block::add(&mut block.one_shots, Some(*sample), amplitude, w.revs, 1.);
                        }
                    }
                }
                Effect::Skid { scale, length, freq, sample } => {
                    for w in [w0, w1] {
                        let (a, v) = (w.lateral_axis, w.slip);
                        let u = (a[2] * v[2] + a[1] * v[1] + a[0] * v[0]).abs();
                        if 0.5 < w.ground && !w.detached && length.threshold_low < u && u < length.threshold_high {
                            // The original's precedence: u - (low / (high - low)).
                            let t = u - length.threshold_low / (length.threshold_high - length.threshold_low);
                            let t = t.clamp(0., 1.);
                            let lerp = (length.high - length.low) * t + length.low;
                            // The lerp goes in the frequency slot and `freq` in the length slot.
                            Block::add(&mut block.loops, Some(*sample), *scale, lerp, *freq);
                        }
                    }
                }
                Effect::Suspension { scale, threshold_low, threshold_high, length, freq, sample } => {
                    for w in [w0, w1] {
                        let v = w.suspension;
                        if 0. < w.ground && !w.detached && *threshold_low < v && v < *threshold_high {
                            Block::add(&mut block.one_shots, Some(*sample), *scale, *freq, *length);
                        }
                    }
                }
                Effect::Collision { scale, threshold_low, threshold_high, freq, length, sample } => {
                    if let Some(dv) = s.collision
                        && *threshold_low < dv
                        && dv < *threshold_high
                    {
                        let (scale, freq, length, name) = (*scale, *freq, *length, sample.clone());
                        let picked = self.setup.pick(&name);
                        Block::add(&mut block.one_shots, picked, scale, freq, length);
                    }
                }
            }
        }
        // 7EA6F0.
        let mut force = (block.force[0] * block.grip[0] + block.force[1] * block.grip[1] + block.camber[1] + block.camber[0])
            * block.gains[0];
        let mut friction = (block.grip[0] + block.grip[1]) * block.gains[1] * block.friction;
        let mut native = false;
        if let Some(d) = self.setup.device(&self.device) {
            force *= d.scale_force;
            friction = clamp_to(d.scale_friction * friction + d.base_friction, d.base_friction, d.max_friction);
            block.gains[2] *= d.scale_effects;
            native = d.native_friction;
        }
        self.output.force = force;
        self.output.friction = friction;
        self.output.native_friction = native;
        self.output.centre = block.centre;
        block
    }
}

/// `7E8580`.
fn caster(c: &Caster, s: &Snapshot, speed: f32, block: &mut Block) {
    let [l, r] = &s.wheels;
    let (trail, camber, toe) = (-c.trail, -c.camber, c.toe);
    if !l.detached {
        block.force[0] = l.lateral * trail + l.longitudinal * toe;
        block.camber[0] = c.scale * camber * l.load;
    }
    if !r.detached {
        block.force[1] = trail * r.lateral + r.longitudinal * -toe;
        block.camber[1] = c.scale * -camber * r.load;
    }
    let mean = (block.force[0] + block.force[1]) * 0.5;
    let a = mean.abs() + c.preload;
    let mut gain = 1.;
    if 0. < a {
        let p = f64::from(a / c.corner_point).powf(f64::from(c.exponent)) as f32;
        gain = (p * c.scale * c.corner_point) / a;
    }
    let shifted = if mean > 0. {
        c.preload + mean
    } else if mean < 0. {
        mean - c.preload
    } else {
        mean
    };
    let force = clamp_to(gain * shifted, -c.scale, c.scale);
    block.force = [force; 2];
    let fade = band(
        &Band {
            threshold_low: c.threshold_low,
            threshold_high: c.threshold_high,
            low: c.friction_low,
            high: c.friction_high,
        },
        speed,
    );
    let mut angle = (s.steer - s.heading).abs() * 57.295_776 - c.angle_threshold_low;
    if angle < 0. {
        angle = 0.;
    }
    angle /= c.angle_threshold_high - c.angle_threshold_low;
    if 1. < angle {
        angle = 1.;
    }
    let load = |w: &WheelSnapshot| w.load.clamp(0., 4.);
    let mut loads = 0.;
    if !l.detached {
        loads = load(l);
    }
    if !r.detached {
        loads += load(r);
    }
    let friction = ((c.angle_friction_high - c.angle_friction_low) * angle + c.angle_friction_low) * loads * 0.5 * fade
        + block.friction;
    block.friction = friction * c.scale;
    block.force = block.force.map(|f| f * fade);
    block.camber = block.camber.map(|f| f * fade);
    let mut f = 1.;
    if c.deadzone_low < c.deadzone_high {
        f = s.heading.abs() * 57.295_776 - c.deadzone_low;
        if f < 0. {
            f = 0.;
        }
        f /= c.deadzone_high - c.deadzone_low;
        if 1. < f {
            f = 1.;
        }
    }
    let centre = s.centre * f;
    let limit = s.centre_limit * 0.95;
    block.centre = if centre < -limit {
        -limit
    } else if limit < centre {
        limit
    } else {
        centre
    };
}

/// `7E7E90`: one sample's value at clock `now` into the channels; false once it has finished.
fn play(setup: &Setup, wheel: bool, p: &Playing, now: f32, channels: &mut [f32; 3]) -> bool {
    let Some(s) = p.sample.and_then(|i| setup.samples.get(i)) else { return false };
    let t = now - p.start;
    if !(0. <= t && t < p.length) {
        return false;
    }
    let index = |count: usize, x: f32| (x.floor().max(0.) as usize).min(15).min(count.max(1) - 1);
    let e = if s.envelopes == 1 {
        s.envelope[0]
    } else {
        s.envelope[index(s.envelopes, (s.envelopes as f32 - 1.) / p.length * t)]
    };
    let mut v = f32::from(e) * 0.111_111;
    v *= if wheel { s.wheel_scale } else { s.rumbler_scale };
    let w = if s.waves == 1 {
        s.wave[0]
    } else {
        let n = s.waves as i64;
        let x = (s.waves as f32 * p.freq * t).floor();
        s.wave[((x as i64).rem_euclid(n) as usize).min(15)]
    };
    v = f32::from(w) * v * 0.25;
    v *= p.amplitude;
    if !wheel && s.channel != 0 {
        channels[1] += v;
    } else {
        channels[0] += v;
    }
    true
}

// ------------------------------------------------------------------------------------- devices

/// `B8CD80`: the DirectInput wheel's software damping and constant-force magnitude.
#[cfg_attr(not(windows), allow(dead_code))] // The DirectInput wheel only.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Damping {
    velocity: f32,
    position: f32,
    damping: f32,
}

impl Damping {
    /// The magnitude for `DICONSTANTFORCE` from the device outputs, the X axis (0..1, raw/65536)
    /// and the FFB tick's dt. [A] clamped to ±10000, DirectInput's nominal range.
    #[cfg_attr(not(windows), allow(dead_code))] // The DirectInput wheel only.
    pub fn magnitude(&mut self, out: &Output, axis: f32, dt: f64) -> i32 {
        let force = out.channels[0] + out.force;
        let friction = out.friction;
        let position = (axis + axis) - 1.;
        let dt = dt as f32;
        self.velocity = if dt > 0. { (((self.velocity + position) - self.position) / dt) * 0.005 } else { 0. };
        self.position = position;
        let mut damping = self.velocity * friction;
        if self.damping * damping < 0. {
            damping = (self.damping + damping) * 0.5;
        }
        self.damping = damping;
        let f = (damping + force) * 10_000.;
        if f.is_nan() { 0 } else { (f as i32).clamp(-10_000, 10_000) }
    }
}

/// `B8CD80` rumble branch: the XInput motor speeds (left, right) from channels 0 and 1.
pub fn motors(out: &Output) -> (u16, u16) {
    let speed = |x: f32| (clamp_to(x, -1., 1.).abs() * 65_535.) as u16;
    (speed(out.channels[0]), speed(out.channels[1]))
}

/// When an Android vibrator is called [A]: the task runs every 10 ms, a call through JNI costs
/// more, and a vibrator cannot follow faster changes. The motor speeds become 8-bit amplitudes;
/// a change is sent at most every 30 ms, a stop at once, and a running level again every 60 ms
/// (the Java side plays 100 ms one-shots).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Default)]
pub struct Vibration {
    sent: (u8, u8),
    at: Option<Instant>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
impl Vibration {
    const CHANGE: Duration = Duration::from_millis(30);
    const REFRESH: Duration = Duration::from_millis(60);

    /// The amplitudes to send now, if any.
    pub fn due(&mut self, now: Instant, motors: (u16, u16)) -> Option<(u8, u8)> {
        let level = ((motors.0 >> 8) as u8, (motors.1 >> 8) as u8);
        let since = self.at.map(|t| now.saturating_duration_since(t));
        let send = if level == (0, 0) {
            self.sent != (0, 0)
        } else if level != self.sent {
            since.is_none_or(|d| d >= Self::CHANGE)
        } else {
            since.is_none_or(|d| d >= Self::REFRESH)
        };
        send.then(|| {
            self.sent = level;
            self.at = Some(now);
            level
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICES: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<FFBDevices>
  <!-- authored test values -->
  <FFBDevice name="default" scaleForce="1.0" scaleFriction="0.8" baseFriction="0.0" maxFriction="10.0" scaleEffects="1.0" delay="0.0"/>
  <FFBDevice name="Pad" scaleEffects="1.0" delay="0.1"/>
  <FFBDevice name="Strong" template="default" scaleForce="1.2" baseFriction="-25.0" maxFriction="45.0"/>
</FFBDevices>"#;
    const CARS: &str = r#"<FFBCars><FFBCar name="a" scaleForce="2.0" scaleFriction="1.0" scaleEffects="1.0"/><FFBCar name="b" template="a" scaleEffects="0.5"/></FFBCars>"#;
    const EFFECTS: &str = r#"<FFBEffects>
    <FFBCaster scale="2.0" toe="0.00" trail="1.0" camber="0.00" angleThresholdLow="6.5" angleFrictionLow="17.5" angleThresholdHigh="25.0" angleFrictionHigh="6.5" thresholdLow="2.5" frictionLow="0.0" thresholdHigh="6.0" frictionHigh="1.0" cornerPoint="0.075" exponent="0.35"/>
    <FFBlateral scale="1.0" thresholdLow="2.5" frictionLow="25" thresholdHigh="6.0" frictionHigh="5"/>
    <FFBCollision scale="1.3" thresholdLow="0.1" thresholdHigh="4.0" damageZone="0" freq="10.0" length="0.25" sample="hit"/>
    <FFBSurface scale="0.5" name="ROAD" grip="1.0" thresholdLow="5.0" thresholdHigh="10.0" freqLow="20.0" freqHigh="40.0" scaleLow="0.0" scaleHigh="1.0" length="0.1" sample="road"/>
    <FFBSurface scale="0.5" name="KERB" grip="0.8" thresholdLow="4.0" thresholdHigh="20.0" freqLow="5.0" freqHigh="15.0" scaleLow="0.2" scaleHigh="1.0" length="0.1" sample="kerb"/>
    <FFBGearChange scale="0.5" gear="2" freq="35.0" length="0.35" sample="gear"/>
    <FFBGearChange scale="0.5" gear="-1" freq="35.0" length="0.22" sample="gear"/>
    <FFBWheelDamage scale="0.3" thresholdLow="0.2" thresholdHigh="1.0" scaleLow="0.0" scaleHigh="1.0" sample="damage"/>
    <FFBSkid scale="0.5" thresholdLow="10" thresholdHigh="45.0" lengthLow="6.66" lengthHigh="4.66" freq="0.15" sample="skid"/>
    <FFBSuspension scale="0.25" thresholdLow="0.75" thresholdHigh="2.0" length="0.1" freq="5" sample="bump"/>
    <FFBSample name="hit" wave="19" envelope="9" />
    <FFBSample name="hit" wave="91" envelope="9" channel="1" />
    <FFBSample name="road" wave="1928" envelope="9876543210987654321" rubmlerScale="1.0" wheelScale = "0.5f"/>
    <FFBSample name="kerb" wave="9" envelope="9" />
    <FFBSample name="gear" wave="1111199999" envelope="9876543210" channel="1" rubmlerScale="1.0" wheelScale="0.3"/>
    <FFBSample name="damage" wave="1955555555" envelope="5"/>
    <FFBSample name="skid" wave="955555555555555" envelope="9" wheelScale="0.1"/>
    <FFBSample name="bump" wave="9" envelope="9564720" channel="1" wheelScale="0.5"/>
</FFBEffects>"#;

    fn setup() -> Setup {
        Setup::parse(DEVICES, EFFECTS, CARS).unwrap()
    }

    #[test]
    fn reads_devices_cars_and_templates() {
        let s = setup();
        let d = s.device("Strong").unwrap();
        assert_eq!((d.scale_force, d.scale_friction, d.base_friction, d.max_friction), (1.2, 0.8, -25., 45.));
        // Constructor values for the attributes a device leaves out.
        let pad = s.device("Pad").unwrap();
        assert_eq!((pad.scale_force, pad.max_friction, pad.delay), (1., 10., 0.1));
        assert_eq!(s.device("FANATEC CSL Elite").unwrap().name, "default");
        assert_eq!(s.cars[1].scale_force, 2.);
        assert_eq!(s.cars[1].scale_effects, 0.5);
    }

    #[test]
    fn reads_samples_like_7e7da0() {
        let s = setup();
        let road = s.samples.iter().find(|x| x.name == "road").unwrap();
        // Wave digits less '5'; 16 envelope digits at most; atof reads "0.5f".
        assert_eq!(&road.wave[..4], &[-4, 4, -3, 3]);
        assert_eq!(road.waves, 4);
        assert_eq!(road.envelopes, 16);
        assert_eq!(road.envelope[15], 4);
        assert_eq!((road.wheel_scale, road.rumbler_scale), (0.5, 1.));
        let gear = s.samples.iter().find(|x| x.name == "gear").unwrap();
        assert_eq!(gear.channel, 1);
    }

    #[test]
    fn effects_keep_the_parser_order() {
        let kinds: Vec<&str> = setup()
            .effects
            .iter()
            .map(|e| match e {
                Effect::Caster(_) => "caster",
                Effect::Lateral { .. } => "lateral",
                Effect::Collision { .. } => "collision",
                Effect::Surface { .. } => "surface",
                Effect::GearChange { .. } => "gear",
                Effect::WheelDamage { .. } => "damage",
                Effect::Skid { .. } => "skid",
                Effect::Suspension { .. } => "suspension",
            })
            .collect();
        assert_eq!(
            kinds,
            ["caster", "lateral", "collision", "surface", "surface", "gear", "gear", "damage", "skid", "suspension"]
        );
    }

    fn snapshot(speed: f32, lateral: f32, surface: Option<u8>) -> Snapshot {
        let wheel = WheelSnapshot {
            lateral_axis: [1., 0., 0., 0.],
            lateral,
            load: 1.,
            surface,
            ground: 1.,
            ..WheelSnapshot::default()
        };
        Snapshot {
            car: "lac".into(),
            velocity: [0., 0., speed, 0.],
            mass: 1200.,
            gear: 3,
            wheels: [wheel; 2],
            ..Snapshot::default()
        }
    }

    /// The caster and lateral friction by hand from §4.1, §4.2 and §4.7 of FORCE_FEEDBACK.md.
    #[test]
    fn caster_and_friction_match_the_formulas() {
        let mut ffb = Ffb::new(setup(), "FANATEC", true);
        // 20 m/s (past the fade), lateral force 0.5 of the static wheel load on both fronts.
        ffb.queue(snapshot(20., 0.5, Some(0)));
        ffb.process(0.01);
        let out = ffb.output();
        let c = -0.5f32; // -trail * lateral
        let g = (f64::from(0.5f32 / 0.075).powf(f64::from(0.35f32)) as f32 * 2. * 0.075) / 0.5;
        let caster = (g * c).clamp(-2., 2.);
        // grip 1 on both wheels, feedback_force 0.5, default scaleForce 1.
        let expected = (caster * 1. + caster * 1. + 0. + 0.) * 0.5;
        assert!((out.force - expected).abs() < 1e-6, "{} vs {expected}", out.force);
        assert!(out.force < 0. && out.force > -0.5);
        // Angle 0 deg: friction 17.5 * (1 + 1) * 0.5 * fade 1, times caster scale 2, plus 5.
        let friction = (17.5f32 * 2. * 0.5 * 1. + 0.) * 2. + 5.;
        let device = ((2. * 0.5 * friction) * 0.8f32).clamp(0., 10.);
        assert_eq!(out.friction, device);
    }

    #[test]
    fn standing_still_has_no_force_but_full_friction() {
        let mut ffb = Ffb::new(setup(), "Strong", true);
        ffb.queue(snapshot(0., 0.5, Some(0)));
        ffb.process(0.01);
        let out = ffb.output();
        assert_eq!(out.force, 0.);
        // fade 0 kills the angle term; the static 25 remains, through the Strong device.
        assert_eq!(out.friction, clamp_to(0.8 * (2. * 0.5 * 25.) - 25., -25., 45.));
    }

    #[test]
    fn an_unknown_surface_gives_no_grip_and_no_force() {
        let mut ffb = Ffb::new(setup(), "x", true);
        ffb.queue(snapshot(20., 0.5, None));
        ffb.process(0.01);
        assert_eq!(ffb.output().force, 0.);
        assert_eq!(ffb.output().friction, 0.);
    }

    #[test]
    fn the_gear_event_skips_neutral_first_and_reverse_pairs() {
        let mut ffb = Ffb::new(setup(), "x", true);
        let mut s = snapshot(20., 0., Some(0));
        for (gear, event) in [(0, None), (1, None), (2, Some(2)), (9, Some(9)), (1, None), (0, None)] {
            s.gear = gear;
            ffb.queue(s.clone());
            assert_eq!(ffb.pending.last().unwrap().gear_event, event, "gear {gear}");
        }
    }

    #[test]
    fn gear_two_plays_its_own_and_the_any_gear_sample() {
        let mut ffb = Ffb::new(setup(), "x", false);
        let mut s = snapshot(0., 0., None);
        s.gear = 1;
        ffb.queue(s.clone());
        ffb.process(0.01);
        s.gear = 2;
        ffb.queue(s);
        ffb.process(0.01);
        assert_eq!(ffb.one_shots.len(), 2);
        // Channel-1 samples go to the right motor of a pad.
        let out = ffb.output();
        assert_eq!(out.channels[0], 0.);
        assert!(out.channels[1] != 0.);
    }

    #[test]
    fn sample_playback_matches_7e7e90() {
        let s = setup();
        let road = s.samples.iter().position(|x| x.name == "road").unwrap();
        let p = Playing { sample: Some(road), amplitude: 2., freq: 20., length: 0.1, start: 1. };
        let mut ch = [0.; 3];
        // t = 0.03: envelope index floor(15 / 0.1 * 0.03) = 4 -> digit 5; wave floor(4 * 20 * 0.03)
        // = 2 -> '2' - '5' = -3.
        assert!(play(&s, true, &p, 1.03, &mut ch));
        let t = 1.03f32 - 1.;
        let e = s.samples[road].envelope[(15. / 0.1 * t).floor() as usize];
        let w = s.samples[road].wave[((4. * 20. * t).floor() as usize) % 4];
        let expected = f32::from(w) * (f32::from(e) * 0.111_111 * 0.5) * 0.25 * 2.;
        assert_eq!(ch[0], expected);
        // Finished once t reaches the length.
        assert!(!play(&s, true, &p, 1.1, &mut ch));
    }

    #[test]
    fn a_collision_band_fires_once_with_a_random_member() {
        let mut ffb = Ffb::new(setup(), "x", true);
        let mut s = snapshot(10., 0., Some(0));
        s.collision = Some(2.);
        ffb.queue(s);
        ffb.process(0.01);
        assert_eq!(ffb.one_shots.len(), 1);
        assert!(ffb.output().channels[0] != 0.);
        // It ends after its 0.25 s.
        for _ in 0..30 {
            ffb.process(0.01);
        }
        assert!(ffb.one_shots.is_empty());
    }

    #[test]
    fn the_skid_swaps_length_and_frequency() {
        let mut ffb = Ffb::new(setup(), "x", true);
        let mut s = snapshot(30., 0., Some(0));
        for w in &mut s.wheels {
            w.slip = [20., 0., 0., 0.];
        }
        let block = ffb.effects(&s);
        let skid = block.loops.iter().find(|p| p.length == 0.15).expect("skid loop");
        // u = 20: t = 20 - 10/35 > 1 -> 1, so the "frequency" is lengthHigh.
        assert_eq!(skid.freq, (4.66f32 - 6.66) * 1. + 6.66);
    }

    #[test]
    fn delay_holds_snapshots_back() {
        let mut ffb = Ffb::new(setup(), "Pad", false);
        ffb.queue(snapshot(20., 0.5, Some(0)));
        ffb.process(0.05);
        assert_eq!(ffb.pending.len(), 1);
        ffb.process(0.06);
        assert!(ffb.pending.is_empty());
    }

    #[test]
    fn disabled_or_paused_outputs_nothing() {
        let mut ffb = Ffb::new(setup(), "x", true);
        ffb.queue(snapshot(20., 0.5, Some(0)));
        ffb.process(0.01);
        assert!(ffb.output().force != 0.);
        ffb.set_paused(true);
        assert_eq!(ffb.output(), Output::default());
        ffb.queue(snapshot(20., 0.5, Some(0)));
        assert!(ffb.pending.is_empty());
        ffb.set_paused(false);
        ffb.set_enabled(false);
        ffb.queue(snapshot(20., 0.5, Some(0)));
        ffb.process(0.01);
        assert_eq!(ffb.output(), Output::default());
    }

    #[test]
    fn damping_opposes_motion_and_averages_on_reversal() {
        let mut d = Damping::default();
        let out = Output { friction: 10., ..Output::default() };
        assert_eq!(d.magnitude(&out, 0.5, 0.01), 0);
        // Moving towards +X: vel = ((0 + 0.02) - 0) / 0.01 * 0.005 = 0.01, damp 0.1 -> 1000.
        let m = d.magnitude(&out, 0.51, 0.01);
        let position = (0.51f32 + 0.51) - 1.;
        let velocity = ((0. + position) - 0.) / 0.01 * 0.005;
        assert_eq!(m, ((velocity * 10. + 0.) * 10_000.) as i32);
        assert!(m > 0);
        // Reversing: the new damping is averaged with the old one.
        let before = d.damping;
        d.magnitude(&out, 0.49, 0.01);
        assert!(d.damping.abs() < before.abs() + 1e-3);
        // dt 0 zeroes the velocity.
        assert_eq!(d.magnitude(&out, 0.3, 0.), 0);
    }

    #[test]
    fn motors_are_the_absolute_channels() {
        let out = Output { channels: [-0.5, 2., 0.], ..Output::default() };
        assert_eq!(motors(&out), (32767, 65535));
    }

    #[test]
    fn vibration_limits_the_calls_and_stops_at_once() {
        let mut v = Vibration::default();
        let t = Instant::now();
        let ms = |n: u64| t + Duration::from_millis(n);
        assert_eq!(v.due(t, (0, 0)), None);
        assert_eq!(v.due(t, (65_535, 0x1234)), Some((255, 0x12)));
        // A change within 30 ms waits; the same level is sent again after 60 ms.
        assert_eq!(v.due(ms(10), (0x8000, 0x1234)), None);
        assert_eq!(v.due(ms(30), (0x8000, 0x1234)), Some((0x80, 0x12)));
        assert_eq!(v.due(ms(80), (0x8000, 0x1234)), None);
        assert_eq!(v.due(ms(90), (0x8000, 0x1234)), Some((0x80, 0x12)));
        // Stop at once, then nothing more.
        assert_eq!(v.due(ms(95), (0x00ff, 0)), Some((0, 0)));
        assert_eq!(v.due(ms(200), (0, 0)), None);
    }

    #[test]
    fn number_reads_like_atof() {
        assert_eq!(number("0.5f"), 0.5);
        assert_eq!(number(" 45.0"), 45.);
        assert_eq!(number("x"), 0.);
        assert_eq!(number("-25"), -25.);
    }

    /// The shipped files, when the game folder is available.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn reads_the_shipped_files() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let s = Setup::load(Path::new(&root)).unwrap();
        assert_eq!(s.device("Logitech G25 Racing Wheel USB").unwrap().max_friction, 45.);
        assert_eq!(s.device("FANATEC").unwrap().name, "default");
        assert!(matches!(s.effects[0], Effect::Caster(c) if c.corner_point == 0.075));
        assert!(s.effects.iter().filter(|e| matches!(e, Effect::Surface { .. })).count() >= 10);
        assert!(s.surface("RUMBLESTRIP").is_some());
    }
}
