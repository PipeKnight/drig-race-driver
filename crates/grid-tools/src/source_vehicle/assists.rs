//! Original driver aids as runtime settings ([research](../../../../docs/DRIVING_ASSISTS_RESEARCH.md)).
//! The options only set player-car bytes; the recovered force pipeline already
//! consumes every one of them. Defaults reproduce the earlier fixed prototype
//! configuration bit for bit: every aid on, automatic, keyboard steering.
use super::{Runtime, byte};
/// The damage option and HUD indicators, for the app (which has no direct
/// grid-world dependency).
pub use grid_world::mechanical_damage;
use grid_world::{Error, mechanical_damage::Damage};

/// Profile option `transmission` (0 automatic, 1 manual). FUN_00606E80 gives
/// the human controller gearbox mode 4 or 2; FUN_0098F110 maps the mode to
/// vehicle +1970 automatic, +1971 clutch assistance, +1972 brake-to-reverse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transmission {
    #[default]
    Automatic,
    Manual,
}

/// Human controller device class (FUN_00827D10 record +1F). Keyboards and
/// pads get assisted steering (+E6C = 1); wheels run unassisted steering and
/// the FUN_00828670 wheel command shaping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Input {
    #[default]
    Keyboard,
    Wheel,
}

/// Single-tick gear request from the original Gear Up/Down and H-shifter
/// actions (FUN_00827D10 actions 6/7 and 0x10..0x16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Up,
    Down,
    /// 1..=6 forward or -1 reverse (vehicle +E2C request).
    Gear(i8),
}

/// Aid strength in percent. The original has on (100) and off (0); 25 and 50
/// are the port's graded steps (AUTHORED, docs/DRIVER_AIDS.md,
/// `grid_world::driver_aids`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level(u8);

impl Level {
    pub const OFF: Self = Self(0);
    pub const ON: Self = Self(100);
    /// The steps the options screen cycles through.
    pub const STEPS: [Self; 4] = [Self(0), Self(25), Self(50), Self(100)];

    pub fn new(percent: u8) -> Option<Self> {
        Self::STEPS.into_iter().find(|l| l.0 == percent)
    }
    pub fn percent(self) -> u8 {
        self.0
    }
    pub fn is_on(self) -> bool {
        self == Self::ON
    }
    /// The vehicle byte: 0 on, 1 off (the original's), graded otherwise.
    pub fn byte(self) -> u8 {
        grid_world::driver_aids::byte(self.0)
    }
    /// The next (or previous) step, wrapping (`wrap="true"`).
    pub fn step(self, up: bool) -> Self {
        let i = Self::STEPS.iter().position(|&l| l == self).unwrap_or(3);
        Self::STEPS[if up { (i + 1) % 4 } else { (i + 3) % 4 }]
    }
    fn parse(key: &str, value: &str) -> Result<Self, String> {
        match value {
            "on" | "true" | "1" | "yes" => Ok(Self::ON),
            "off" | "false" | "no" => Ok(Self::OFF),
            v => v
                .trim_end_matches('%')
                .parse()
                .ok()
                .and_then(Self::new)
                .ok_or_else(|| format!("assist `{key}` needs on, off, 0, 25, 50 or 100")),
        }
    }
}

impl From<bool> for Level {
    fn from(on: bool) -> Self {
        if on { Self::ON } else { Self::OFF }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            100 => f.write_str("on"),
            0 => f.write_str("off"),
            p => write!(f, "{p}%"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assists {
    /// Option `driveraid_traction`; off sets +155A. 25/50 % are graded
    /// (AUTHORED).
    pub traction_control: Level,
    /// Option `driveraid_steering`, shown as Stability Control; off sets +1558.
    pub stability_control: bool,
    /// Option `driveraid_brake`, shown as Braking Assist; off sets +1559.
    /// 25/50 % are graded (AUTHORED).
    pub braking_assist: Level,
    pub transmission: Transmission,
    pub input: Input,
    /// Wheel response record +14: the constructor's 0.4 unless the
    /// `wheelLinearity` controller option L sets (L - 0.5) * 2.
    pub wheel_response: f32,
    /// Option `realistic_damage` (on by default in a fresh profile); off is
    /// the online session's damage-off override.
    pub damage: Damage,
}

impl Default for Assists {
    fn default() -> Self {
        Self {
            traction_control: Level::ON,
            stability_control: true,
            braking_assist: Level::ON,
            transmission: Transmission::Automatic,
            input: Input::Keyboard,
            wheel_response: f32::from_bits(0x3ecc_cccd),
            damage: Damage::Realistic,
        }
    }
}

impl Assists {
    /// Every aid off; the original applies this to all driver objects when
    /// the event disables assists (FUN_006572D0).
    pub fn none() -> Self {
        Self {
            traction_control: Level::OFF,
            stability_control: false,
            braking_assist: Level::OFF,
            transmission: Transmission::Manual,
            ..Self::default()
        }
    }

    pub fn gearbox_mode(&self) -> u8 {
        match self.transmission {
            Transmission::Automatic => 4,
            Transmission::Manual => 2,
        }
    }

    /// Player-car bytes, written as FUN_00654C20 and FUN_00828670 do.
    pub fn apply(&self, vehicle: &mut [u32]) {
        byte(vehicle, 0x1558, u8::from(!self.stability_control));
        byte(vehicle, 0x1559, self.braking_assist.byte());
        byte(vehicle, 0x155a, self.traction_control.byte());
        let [automatic, clutch, reverse] = gearbox_bytes(self.gearbox_mode());
        byte(vehicle, 0x1970, automatic);
        byte(vehicle, 0x1971, clutch);
        byte(vehicle, 0x1972, reverse);
        byte(vehicle, 0xe6c, u8::from(self.input == Input::Keyboard));
        self.damage.apply(vehicle);
    }

    /// `key=value` pairs separated by commas, spaces or new lines; `#` starts a
    /// comment. Keys: tcs/traction, stability, braking/abs, gearbox, input,
    /// linearity (option value 0..1) or response (record value -1..1), all
    /// (on/off), preset (default/none), damage (realistic/normal/off).
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut assists = Self::default();
        for item in spec
            .lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .flat_map(|l| l.split([',', ' ', '\t', ';']))
            .filter(|s| !s.is_empty())
        {
            let (key, value) = item
                .split_once('=')
                .ok_or_else(|| format!("assist setting `{item}` needs key=value"))?;
            let (key, value) = (key.to_ascii_lowercase(), value.to_ascii_lowercase());
            let on = || match value.as_str() {
                "on" | "true" | "1" | "yes" => Ok(true),
                "off" | "false" | "0" | "no" => Ok(false),
                _ => Err(format!("assist `{key}` needs on or off")),
            };
            match key.as_str() {
                "tcs" | "traction" | "traction_control" => {
                    assists.traction_control = Level::parse(&key, &value)?
                }
                "stability" | "stability_control" | "scs" | "esp" => {
                    assists.stability_control = on()?
                }
                "braking" | "braking_assist" | "brake" | "abs" => {
                    assists.braking_assist = Level::parse(&key, &value)?
                }
                "gearbox" | "transmission" | "gears" => {
                    assists.transmission = match value.as_str() {
                        "auto" | "automatic" => Transmission::Automatic,
                        "manual" => Transmission::Manual,
                        _ => return Err("gearbox needs automatic or manual".into()),
                    }
                }
                "input" | "device" => {
                    assists.input = match value.as_str() {
                        "keyboard" | "pad" | "gamepad" => Input::Keyboard,
                        "wheel" => Input::Wheel,
                        _ => return Err("input needs keyboard, pad or wheel".into()),
                    }
                }
                "linearity" => {
                    let l: f32 = value
                        .parse()
                        .map_err(|_| "linearity needs a number 0..1".to_string())?;
                    if !(0. ..=1.).contains(&l) {
                        return Err("linearity needs a number 0..1".into());
                    }
                    assists.wheel_response = (l - 0.5) * 2.;
                }
                "response" => {
                    let r: f32 = value
                        .parse()
                        .map_err(|_| "response needs a number -1..1".to_string())?;
                    if !(-1. ..=1.).contains(&r) {
                        return Err("response needs a number -1..1".into());
                    }
                    assists.wheel_response = r;
                }
                "damage" | "realistic_damage" => {
                    assists.damage = match value.as_str() {
                        "realistic" | "on" | "2" => Damage::Realistic,
                        "normal" | "1" => Damage::Normal,
                        "off" | "none" | "0" => Damage::Off,
                        _ => return Err("damage needs realistic, normal or off".into()),
                    }
                }
                "all" => {
                    let on = on()?;
                    assists.traction_control = on.into();
                    assists.stability_control = on;
                    assists.braking_assist = on.into();
                }
                "preset" => {
                    assists = match value.as_str() {
                        "default" => Self::default(),
                        "none" | "pro" => Self::none(),
                        _ => return Err("preset needs default or none".into()),
                    }
                }
                _ => return Err(format!("unknown assist setting `{key}`")),
            }
        }
        Ok(assists)
    }
}

impl std::fmt::Display for Assists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let on = |b: bool| if b { "on" } else { "off" };
        write!(
            f,
            "tcs={},stability={},braking={},gearbox={},input={}",
            self.traction_control,
            on(self.stability_control),
            self.braking_assist,
            match self.transmission {
                Transmission::Automatic => "automatic",
                Transmission::Manual => "manual",
            },
            match self.input {
                Input::Keyboard => "keyboard",
                Input::Wheel => "wheel",
            }
        )?;
        if self.input == Input::Wheel {
            write!(f, ",response={}", self.wheel_response)?;
        }
        write!(
            f,
            ",damage={}",
            match self.damage {
                Damage::Realistic => "realistic",
                Damage::Normal => "normal",
                Damage::Off => "off",
            }
        )
    }
}

/// FUN_0098F110 table: +1970 automatic, +1971 clutch, +1972 brake-to-reverse.
/// Modes 0, 1 and 3 exist but no player path selects them.
pub fn gearbox_bytes(mode: u8) -> [u8; 3] {
    match mode {
        0 => [1, 0, 0],
        2 => [0, 1, 0],
        3 => [1, 1, 0],
        4 => [1, 1, 1],
        _ => [0, 0, 0],
    }
}

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn v(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
// SSE horizontal sum order of the original dot products.
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[2] * b[2] + a[3] * b[3]) + (a[0] * b[0] + a[1] * b[1])
}

/// FUN_009AE5A0: blend of the front-axle velocity heading and the current
/// wheel angle (+E54 * +E30), weighted by speed between 2 and 4 m/s.
pub fn front_heading(b: &[u32]) -> f32 {
    let (w2, w3) = (v(b, 0x1c30), v(b, 0x1d80));
    let p: [f32; 4] = [
        (w3[0] + w2[0]) * 0.5,
        (w3[1] + w2[1]) * 0.5,
        (w3[2] + w2[2]) * 0.5,
        1.,
    ];
    let rows = [v(b, 0x15e0), v(b, 0x15f0), v(b, 0x1600), v(b, 0x1610)];
    let r: [f32; 4] = std::array::from_fn(|i| {
        (p[3] * rows[3][i] + p[1] * rows[1][i]) + (p[2] * rows[2][i] + p[0] * rows[0][i])
            - rows[3][i]
    });
    let a = v(b, 0x15d0);
    let cross = [
        a[1] * r[2] - a[2] * r[1],
        a[2] * r[0] - a[0] * r[2],
        a[0] * r[1] - a[1] * r[0],
        0.,
    ];
    let linear = v(b, 0x15c0);
    let velocity: [f32; 4] = std::array::from_fn(|i| linear[i] + cross[i]);
    let forward = dot(rows[2], velocity);
    let lateral = dot(rows[0], velocity);
    let t = ((f(b, 0x1648).abs() - 2.) * 0.5).clamp(0., 1.);
    let angle = f(b, 0xe54) * f(b, 0xe30);
    if t > 0. {
        let heading = (f64::from(lateral) / f64::from(forward)).atan();
        (heading * f64::from(t) + (1. - f64::from(t)) * f64::from(angle)) as f32
    } else {
        angle
    }
}

/// FUN_00828670 wheel branch (human, record +1F clear, gearbox mode != 1).
/// `command` and the result use the vehicle +E30 sign (the record's negation).
pub fn wheel_command(b: &[u32], command: f32, response: f32) -> f32 {
    let s = -command;
    let ratio = if f(b, 0x1384) > 50. { 1. } else { 1.5_f32 };
    let scaled = s * (1. / ratio);
    let current = f64::from(front_heading(b)) / f64::from(f(b, 0xe54));
    let rounded = current as f32;
    let a = (current + f64::from(s)) as f32;
    let k = if response >= 0. {
        response * 0.5 + 1.
    } else {
        1. / ((1. - response) * 0.5)
    };
    let r = if a >= 0. {
        let r = (f64::from(a).powf(f64::from(k)) as f32) - rounded;
        if -1. > r {
            -1.
        } else if r > scaled {
            scaled
        } else {
            r
        }
    } else {
        let p = f64::from(-0.0 - a).powf(f64::from(k)) as f32;
        let r = (-0.0 - p) - rounded;
        if scaled > r {
            scaled
        } else if r > 1. {
            1.
        } else {
            r
        }
    };
    -r
}

impl Runtime {
    /// Apply driver aids before the first tick; reset keeps them.
    pub fn set_assists(&mut self, assists: Assists) -> Result<(), Error> {
        if self.ticks != 0 || !assists.wheel_response.is_finite() {
            return Err(Error("driver aids change only before the first tick"));
        }
        for state in [&mut self.state, std::sync::Arc::make_mut(&mut self.initial)] {
            assists.apply(&mut state.car.owner.vehicle);
            assists.apply(&mut state.car.frame.body);
        }
        self.assists = assists;
        Ok(())
    }

    /// One-shot request consumed by the next tick (the original record writes
    /// slots 3/4/5 every frame). Automatic gearboxes clear it unread.
    pub fn request_shift(&mut self, shift: Shift) -> Result<(), Error> {
        if let Shift::Gear(g) = shift
            && !(g == -1 || (1..=6).contains(&g))
        {
            return Err(Error("gear request needs 1..6 or -1"));
        }
        self.shift = Some((self.ticks, shift));
        Ok(())
    }

    /// AI control packet for the next tick: the reverse byte +0x1E that the
    /// AI publishes with gearbox mode 3 (docs/AI_AVOID_RECOVERY_RESEARCH.md §5).
    pub fn set_ai_packet(&mut self, reverse: bool) {
        self.ai_packet = Some((self.ticks, reverse));
    }

    /// Current gear: 0 neutral, 1..8 forward, 9 reverse (vehicle +196C).
    pub fn gear(&self) -> u32 {
        self.state.car.owner.vehicle[0x196c / 4]
    }

    /// Input-record stage of a tick: gear slots and the device's command.
    pub(super) fn stage_driver(
        &self,
        staged: &mut grid_world::vehicle_manager::State,
        steering: f32,
    ) -> f32 {
        let shift = self
            .shift
            .filter(|(tick, _)| *tick == self.ticks)
            .map(|s| s.1);
        let ai = self
            .ai_packet
            .filter(|(tick, _)| *tick == self.ticks)
            .map(|p| p.1);
        // FUN_00828670 passes the packet's gearbox mode (+0x24) to
        // FUN_0098F110 every frame: 3 from the AI (0x794310), the player's
        // option otherwise.
        let mode = if ai.is_some() {
            3
        } else {
            self.assists.gearbox_mode()
        };
        let [automatic, clutch, reverse] = gearbox_bytes(mode);
        for b in [
            &mut staged.owner.vehicle[..],
            &mut staged.frame.body[..],
        ] {
            byte(b, 0xe24, u8::from(shift == Some(Shift::Up)));
            byte(b, 0xe28, u8::from(shift == Some(Shift::Down)));
            b[0xe2c / 4] = match shift {
                Some(Shift::Gear(g)) => i32::from(g) as u32,
                _ => 0,
            };
            // The reverse byte selects reverse (0x98E190 refuses it while
            // +21F2 is set); without it an AI packet takes a car in reverse
            // or neutral back to first gear (0x8289EB).
            match ai {
                Some(true) if (b[0x21f0 / 4] >> 16) & 255 == 0 => b[0x196c / 4] = 9,
                Some(false) if matches!(b[0x196c / 4], 0 | 9) => b[0x196c / 4] = 1,
                _ => {}
            }
            byte(b, 0x1970, automatic);
            byte(b, 0x1971, clutch);
            byte(b, 0x1972, reverse);
        }
        match self.assists.input {
            Input::Keyboard => steering,
            Input::Wheel => wheel_command(
                &staged.owner.vehicle,
                steering,
                self.assists.wheel_response,
            ),
        }
    }
}

impl super::multi::MultiRuntime {
    /// Driver aids for one car (the original applies options to the player).
    pub fn set_assists(&mut self, car: usize, assists: Assists) -> Result<(), Error> {
        self.cars
            .get_mut(car)
            .ok_or(Error("no such car"))?
            .set_assists(assists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_parse_display_and_bytes() {
        let d = Assists::default();
        assert_eq!(Assists::parse("").unwrap(), d);
        assert_eq!(d.gearbox_mode(), 4);
        assert_eq!(
            d.to_string(),
            "tcs=on,stability=on,braking=on,gearbox=automatic,input=keyboard,damage=realistic"
        );
        let mut b = vec![0u32; 0x2000 / 4];
        d.apply(&mut b);
        // FUN_00654C20 damage stores (the body copy has no mutable block).
        assert_eq!((b[0x155c / 4], b[0xb14 / 4]), (2, 1));
        for (spec, damage, mode) in [
            ("damage=normal", Damage::Normal, 1),
            ("damage=off", Damage::Off, 0),
            ("realistic_damage=on", Damage::Realistic, 2),
        ] {
            let a = Assists::parse(spec).unwrap();
            assert_eq!(a.damage, damage);
            assert_eq!(Assists::parse(&a.to_string()).unwrap(), a);
            let mut c = vec![0u32; 0x2000 / 4];
            a.apply(&mut c);
            assert_eq!(c[0x155c / 4], mode);
            assert_eq!(c[0xb14 / 4], u32::from(mode != 0));
        }
        assert!(Assists::parse("damage=cosmetic").is_err());
        // The earlier fixed prototype bytes: 1970/1971/1972 set, aids clear.
        assert_eq!(b[0x1970 / 4], 0x0001_0101);
        assert_eq!(b[0x1558 / 4], 0);
        assert_eq!(b[0xe6c / 4], 1);
        let a = Assists::parse("abs=on,tcs=off,stability=off,gearbox=manual").unwrap();
        assert!(!a.traction_control.is_on() && !a.stability_control && a.braking_assist.is_on());
        assert_eq!(a.gearbox_mode(), 2);
        a.apply(&mut b);
        assert_eq!(b[0x1558 / 4], 0x0001_0001);
        assert_eq!(b[0x1970 / 4], 0x0000_0100);
        let w = Assists::parse("# file\ninput=wheel linearity=1\nall=off").unwrap();
        assert_eq!(w.wheel_response, 1.);
        assert_eq!(Assists::parse(&w.to_string()).unwrap().input, Input::Wheel);
        w.apply(&mut b);
        assert_eq!(b[0xe6c / 4] & 255, 0);
        assert_eq!(b[0x1558 / 4], 0x0001_0101);
        assert_eq!(Assists::parse("preset=none").unwrap(), Assists::none());
        for bad in ["tcs", "tcs=maybe", "gearbox=cvt", "linearity=2", "turbo=on"] {
            assert!(Assists::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn graded_levels_parse_display_and_bytes() {
        let a = Assists::parse("tcs=25 braking=50%").unwrap();
        assert_eq!(a.traction_control.percent(), 25);
        assert_eq!(a.braking_assist.percent(), 50);
        assert_eq!(Assists::parse(&a.to_string()).unwrap(), a);
        assert!(a.to_string().starts_with("tcs=25%,stability=on,braking=50%"));
        // 0 and 100 are the original's off and on, with their bytes 1 and 0.
        let off = Assists::parse("tcs=0,braking=0").unwrap();
        assert_eq!(off, Assists::parse("tcs=off,braking=off").unwrap());
        assert_eq!(Assists::parse("tcs=100").unwrap(), Assists::default());
        let mut b = vec![0u32; 0x2000 / 4];
        off.apply(&mut b);
        assert_eq!(b[0x1558 / 4], 0x0001_0100);
        a.apply(&mut b);
        assert_eq!(b[0x1558 / 4], (0x80 | 75) << 16 | (0x80 | 50) << 8);
        assert!(Assists::parse("tcs=30").is_err());
        // Right cycles off, 25, 50, on and wraps.
        let steps: Vec<u8> = std::iter::successors(Some(Level::OFF), |l| Some(l.step(true)))
            .take(5)
            .map(Level::percent)
            .collect();
        assert_eq!(steps, [0, 25, 50, 100, 0]);
        assert_eq!(Level::OFF.step(false), Level::ON);
    }

    #[test]
    fn gearbox_table_matches_98f110() {
        assert_eq!(gearbox_bytes(0), [1, 0, 0]);
        assert_eq!(gearbox_bytes(1), [0, 0, 0]);
        assert_eq!(gearbox_bytes(2), [0, 1, 0]);
        assert_eq!(gearbox_bytes(3), [1, 1, 0]);
        assert_eq!(gearbox_bytes(4), [1, 1, 1]);
    }

    fn straight(speed: f32, command: f32) -> Vec<u32> {
        let mut b = vec![0u32; 0x2000 / 4];
        let mut set = |o: usize, x: f32| b[o / 4] = x.to_bits();
        // Identity pose, front axle 1.3 m ahead, wheels at +/-0.74 m.
        for (row, o) in [0x15e0, 0x15f0, 0x1600, 0x1610].into_iter().enumerate() {
            if row < 3 {
                set(o + row * 4, 1.);
            }
        }
        set(0x1610 + 12, 1.);
        set(0x1c30, 0.74);
        set(0x1c38, 1.3);
        set(0x1d80, -0.74);
        set(0x1d88, 1.3);
        set(0x15c8, speed);
        set(0x1648, speed);
        set(0xe54, 0.576);
        set(0xe30, command);
        set(0x1384, 14.);
        b
    }

    #[test]
    fn wheel_shaping_bounds_and_symmetry() {
        // Below 2 m/s the heading is the commanded wheel angle.
        let slow = straight(1., 0.5);
        assert_eq!(front_heading(&slow), 0.576 * 0.5);
        // Moving straight, the heading term vanishes; linear response (k = 1)
        // reduces to the 1/1.5 wheel ratio, the clamp's input-side bound.
        let b = straight(20., 0.);
        assert_eq!(front_heading(&b), 0.);
        for c in [-1_f32, -0.5, 0., 0.25, 1.] {
            let r = wheel_command(&b, c, 0.);
            assert_eq!(r, -((-c) * (1. / 1.5)), "command {c}");
            assert_eq!(wheel_command(&b, -c, 0.), -r);
        }
        // A positive response raises the exponent: small inputs soften.
        assert!(wheel_command(&b, -0.3, 1.).abs() < wheel_command(&b, -0.3, 0.).abs());
        // Sliding sideways, the shaped target is relative to the axle heading
        // and stays inside the clamp [-1, scaled input].
        let mut slide = straight(20., 0.);
        slide[0x15c0 / 4] = 10f32.to_bits();
        assert!(front_heading(&slide) > 0.);
        for c in [-1_f32, -0.2, 0.2, 1.] {
            let r = -wheel_command(&slide, c, 1.);
            assert!(r.is_finite() && (-1. ..=1.).contains(&r), "{c} {r}");
        }
    }

    fn f32_at(r: &Runtime, o: usize) -> f32 {
        f32::from_bits(r.state.car.owner.vehicle[o / 4])
    }

    fn surface_speed(r: &Runtime, i: usize) -> f32 {
        f32_at(r, 0x1990 + i * 0x150 + 0x104) * f32_at(r, 0x1990 + i * 0x150 + 0xe0)
    }

    fn speed(r: &Runtime) -> f32 {
        let v = r.velocity();
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    #[test]
    #[ignore = "requires purchased GRID sources"]
    fn source_ai_packet_reverse() {
        use crate::source_vehicle::Controls;
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = std::path::Path::new(&root);
                let at = |r: &Runtime| {
                    let p = r.pose();
                    [p[12], p[13], p[14]]
                };
                let mut forward = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                let mut r = forward.clone();
                let start = at(&r);
                let pedal = |throttle, brake| Controls {
                    throttle,
                    brake,
                    ..Default::default()
                };
                // Mode 3: braking at rest never selects reverse.
                for _ in 0..60 {
                    r.set_ai_packet(false);
                    r.tick(pedal(0., 1.)).unwrap();
                    assert_eq!(r.gear(), 1);
                }
                assert_eq!(r.state.car.owner.vehicle[0x1970 / 4] & 0xff_ffff, 0x0101);
                // The reverse byte selects reverse; the throttle drives backwards.
                for _ in 0..120 {
                    r.set_ai_packet(true);
                    r.tick(pedal(0.5, 0.)).unwrap();
                    assert_eq!(r.gear(), 9);
                }
                for _ in 0..180 {
                    forward.tick(pedal(0.5, 0.)).unwrap();
                }
                let d = |p: [f32; 3]| [p[0] - start[0], p[1] - start[1], p[2] - start[2]];
                let (back, ahead) = (d(at(&r)), d(at(&forward)));
                assert!(
                    back[0] * ahead[0] + back[2] * ahead[2] < -1.,
                    "{back:?} {ahead:?}"
                );
                // Clearing the byte returns to first gear; without a packet
                // the human gearbox mode (4 by default) is back.
                r.set_ai_packet(false);
                r.tick(pedal(0., 1.)).unwrap();
                assert_eq!(r.gear(), 1);
                r.tick(pedal(0., 0.)).unwrap();
                assert_eq!(r.state.car.owner.vehicle[0x1970 / 4] & 0xff_ffff, 0x01_0101);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID sources"]
    fn source_driving_assists_matrix() {
        use crate::source_vehicle::Controls;
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = std::path::Path::new(&root);
                let new = |a: Assists| {
                    let mut r = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                    r.set_assists(a).unwrap();
                    r
                };
                let throttle = Controls {
                    throttle: 1.,
                    ..Default::default()
                };
                // Default aids with damage=normal are the earlier fixed
                // configuration bit for bit, apart from the mode word +155C.
                let mut plain = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                plain.state.car.owner.vehicle[0x155c / 4] = 1;
                plain.state.car.frame.body[0x155c / 4] = 1;
                let mut set = new(Assists {
                    damage: Damage::Normal,
                    ..Assists::default()
                });
                // The default Realistic Damage doubles every damage gain; with
                // nothing hit the driving is unchanged.
                let mut realistic = new(Assists::default());
                for tick in 0..300 {
                    let c = Controls {
                        steering: if tick > 150 { 0.4 } else { 0. },
                        ..throttle
                    };
                    plain.tick(c).unwrap();
                    set.tick(c).unwrap();
                    realistic.tick(c).unwrap();
                    assert_eq!(plain.state.car.owner.vehicle, set.state.car.owner.vehicle);
                    assert_eq!(plain.state.car.owner.physical, set.state.car.owner.physical);
                    assert_eq!(
                        plain.state.world.groups.contacts,
                        set.state.world.groups.contacts
                    );
                    assert_eq!(
                        plain.state.car.owner.physical,
                        realistic.state.car.owner.physical
                    );
                }
                assert!(set.set_assists(Assists::none()).is_err());
                // Traction control off: driven front wheels spin up harder at launch.
                let launch = |a: Assists| {
                    let mut r = new(a);
                    let mut slip = 0f32;
                    for _ in 0..250 {
                        r.tick(throttle).unwrap();
                        let s = speed(&r);
                        if s > 1. {
                            slip = slip
                                .max((surface_speed(&r, 2) + surface_speed(&r, 3)) * 0.5 / s - 1.);
                        }
                    }
                    (slip, speed(&r))
                };
                let on = launch(Assists::default());
                let off = launch(Assists {
                    traction_control: Level::OFF,
                    ..Default::default()
                });
                eprintln!("assists_launch tcs_on={on:?} tcs_off={off:?}");
                assert!(off.0 > on.0);
                // Braking assist off: a sustained full brake runs the lock-up timer
                // past +1168 and stops the wheels while the car still moves.
                let brake = |a: Assists| {
                    let mut r = new(a);
                    for _ in 0..375 {
                        r.tick(throttle).unwrap();
                    }
                    let (mut timer, mut locked, mut stop) = (0f32, 0, None);
                    for t in 0..300 {
                        r.tick(Controls {
                            brake: 1.,
                            ..Default::default()
                        })
                        .unwrap();
                        timer = timer.max(f32_at(&r, 0xe74));
                        if speed(&r) > 2. && (0..4).any(|i| surface_speed(&r, i).abs() < 0.05) {
                            locked += 1;
                        }
                        if stop.is_none() && speed(&r) < 0.5 {
                            stop = Some(t);
                        }
                    }
                    (timer, locked, stop, r.gear())
                };
                let on = brake(Assists::default());
                let off = brake(Assists {
                    braking_assist: Level::OFF,
                    ..Default::default()
                });
                eprintln!("assists_brake ba_on={on:?} ba_off={off:?}");
                assert_eq!((on.0, on.1), (0., 0));
                assert!(off.0 > 0.3 && off.1 > 0);
                // Graded steps (AUTHORED): between on and off.
                let level = |p| Level::new(p).unwrap();
                let tc: Vec<_> = [25, 50]
                    .map(|p| {
                        launch(Assists {
                            traction_control: level(p),
                            ..Default::default()
                        })
                    })
                    .into();
                let ba: Vec<_> = [25, 50]
                    .map(|p| {
                        brake(Assists {
                            braking_assist: level(p),
                            ..Default::default()
                        })
                    })
                    .into();
                eprintln!("assists_graded tcs_25_50={tc:?} ba_25_50={ba:?}");
                let (tc_on, tc_off) = (launch(Assists::default()).0, launch(Assists {
                    traction_control: Level::OFF,
                    ..Default::default()
                }).0);
                for (slip, _) in &tc {
                    assert!(*slip > tc_on && *slip < tc_off, "{slip} {tc_on} {tc_off}");
                }
                assert!(tc[0].0 > tc[1].0, "25 % slips more than 50 %");
                // The lock-up comes later, so the car stops later than with
                // the aid off and sooner than with it on.
                assert!(off.2 < ba[0].2 && ba[0].2 < ba[1].2 && ba[1].2 < on.2, "{ba:?} {off:?} {on:?}");
                // 0 % is the original's off and 100 % its on, bit for bit: the
                // same run as with the bytes written as the original does.
                let mut graded = new(Assists::parse("tcs=0 braking=0").unwrap());
                let mut original = new(Assists::default());
                for b in [
                    &mut original.state.car.owner.vehicle[..],
                    &mut original.state.car.frame.body[..],
                ] {
                    byte(b, 0x1559, 1);
                    byte(b, 0x155a, 1);
                }
                let mut hundred = new(Assists::parse("tcs=100 braking=100").unwrap());
                let mut default = new(Assists::default());
                for tick in 0..700 {
                    let c = Controls {
                        throttle: if tick < 400 { 1. } else { 0. },
                        brake: if tick < 400 { 0. } else { 1. },
                        steering: if (200..300).contains(&tick) { 0.3 } else { 0. },
                        ..Default::default()
                    };
                    for r in [&mut graded, &mut original, &mut hundred, &mut default] {
                        r.tick(c).unwrap();
                    }
                    assert_eq!(graded.state.car.owner.vehicle, original.state.car.owner.vehicle);
                    assert_eq!(graded.state.car.owner.physical, original.state.car.owner.physical);
                    assert_eq!(hundred.state.car.owner.vehicle, default.state.car.owner.vehicle);
                    assert_eq!(hundred.state.car.owner.physical, default.state.car.owner.physical);
                }
                // Stability control off: less front grip (Lacetti x0.6), a lower yaw
                // rate while turning in under power. (Measured in the steady turn
                // before F417, where on/off differed by under 1 % either way.)
                let turn = |a: Assists| {
                    let mut r = new(a);
                    let mut yaw = 0f32;
                    for t in 0..260 {
                        let c = if t < 190 {
                            throttle
                        } else {
                            Controls {
                                throttle: 0.7,
                                steering: 1.,
                                ..Default::default()
                            }
                        };
                        r.tick(c).unwrap();
                        if (190..260).contains(&t) {
                            yaw += r.angular_velocity()[1].abs();
                        }
                    }
                    yaw / 70.
                };
                let on = turn(Assists::default());
                let off = turn(Assists {
                    stability_control: false,
                    ..Default::default()
                });
                eprintln!("assists_turn sc_on={on} sc_off={off}");
                assert!(off < on);
                // Manual gearbox: no automatic shifts, X/Z requests, N and R.
                let mut m = new(Assists {
                    transmission: Transmission::Manual,
                    ..Default::default()
                });
                for _ in 0..400 {
                    m.tick(throttle).unwrap();
                }
                assert_eq!(m.gear(), 1);
                m.request_shift(Shift::Up).unwrap();
                m.tick(throttle).unwrap();
                assert_eq!(m.gear(), 2);
                m.tick(throttle).unwrap();
                assert_eq!(m.gear(), 2);
                m.request_shift(Shift::Gear(4)).unwrap();
                m.tick(throttle).unwrap();
                assert_eq!(m.gear(), 4);
                let mut r = new(Assists {
                    transmission: Transmission::Manual,
                    ..Default::default()
                });
                r.tick(Controls::default()).unwrap();
                r.request_shift(Shift::Down).unwrap();
                r.tick(Controls::default()).unwrap();
                assert_eq!(r.gear(), 0);
                for _ in 0..60 {
                    r.tick(throttle).unwrap();
                }
                assert!(speed(&r) < 0.5);
                r.request_shift(Shift::Down).unwrap();
                r.tick(Controls::default()).unwrap();
                assert_eq!(r.gear(), 9);
                for _ in 0..120 {
                    r.tick(throttle).unwrap();
                }
                let reverse = r.velocity();
                let forward: [f32; 3] = std::array::from_fn(|i| f32_at(&r, 0x1600 + i * 4));
                assert!((0..3).map(|i| reverse[i] * forward[i]).sum::<f32>() < -0.5);
                r.reset();
                assert_eq!(r.gear(), 1);
                // Wheel input: unassisted steering and the shaped command.
                let mut w = new(Assists {
                    input: Input::Wheel,
                    ..Default::default()
                });
                assert_eq!(w.state.car.owner.vehicle[0xe6c / 4] & 255, 0);
                for _ in 0..200 {
                    w.tick(throttle).unwrap();
                }
                let expected =
                    wheel_command(&w.state.car.owner.vehicle, 0.5, w.assists.wheel_response);
                w.tick(Controls {
                    steering: 0.5,
                    ..throttle
                })
                .unwrap();
                assert_eq!(f32_at(&w, 0xe30), expected);
                assert!(expected != 0.5 && expected > 0.);
                eprintln!("assists_wheel input=0.5 command={expected}");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
