//! Joined four-wheel, mode-one force solve, active limits and event advancement.
//! Supplied prepared vectors/contact state; no contact detection or pose advance.
use crate::{Error, dynamics, force_equations::System};

pub const BODY_WORDS: usize = 0x2600 / 4;
pub const VIEW_WORDS: usize = 0x2b0 / 4;
const EPSILON: f32 = f32::from_bits(0x3727c5ac);

/// Bounded diagnostic layout. Opaque words survive; no stored address is followed.
#[derive(Clone, Debug)]
pub struct Frame {
    pub body: [u32; BODY_WORDS],
    pub view: [u32; VIEW_WORDS],
    pub solver: [u32; 884],
    pub driven: Vec<usize>,
    pub material_multipliers: [Option<f32>; 4],
    pub diagnostics: [f32; 8],
}

fn f(words: &[u32], offset: usize) -> f32 {
    f32::from_bits(words[offset / 4])
}
fn set(words: &mut [u32], offset: usize, value: f32) {
    words[offset / 4] = value.to_bits();
}
fn u(words: &[u32], offset: usize) -> usize {
    words[offset / 4] as usize
}
fn integer(words: &mut [u32], offset: usize, value: usize) {
    words[offset / 4] = value as u32;
}
fn byte(words: &[u32], offset: usize) -> u8 {
    (words[offset / 4] >> ((offset % 4) * 8)) as u8
}
fn set_byte(words: &mut [u32], offset: usize, value: u8) {
    let shift = (offset % 4) * 8;
    words[offset / 4] = (words[offset / 4] & !(255 << shift)) | ((value as u32) << shift);
}
fn wheel(index: usize) -> usize {
    0x1990 + index * 0x150
}
fn vector(words: &[u32], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(words, offset + i * 4))
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[3] * b[3] + a[2] * b[2]) + (a[1] * b[1] + a[0] * b[0])
}

impl Frame {
    fn id(&self, offset: usize, index: usize) -> usize {
        u(&self.view, offset + index * 4)
    }
    fn value(&self, system: &System, offset: usize, index: usize) -> Result<f32, Error> {
        system
            .value(self.id(offset, index))
            .ok_or(Error("invalid vehicle force variable"))
    }
    fn row(
        &mut self,
        system: &mut System,
        offset: usize,
        index: usize,
        terms: &[(usize, f32)],
        rhs: f32,
    ) -> Result<(), Error> {
        let id = system.logical_count();
        system.add(terms, rhs)?;
        integer(&mut self.view, offset + 4 * index, id);
        Ok(())
    }
    fn initialize(&mut self) {
        let ratio = f(&self.body, 0x18f4 + 4 * u(&self.body, 0x196c));
        let ratio = if ratio == 0. { 1. } else { ratio };
        let a = f(&self.view, 0x58);
        let b = f(&self.view, 0x9c);
        set(&mut self.view, 0x290, if b < a { b } else { a });
        set(
            &mut self.view,
            0x294,
            f(&self.body, 0x17a0) / (ratio * ratio),
        );
        integer(
            &mut self.view,
            4,
            usize::from(f(&self.body, 0xe34) == 1. || u(&self.body, 0x196c) == 0),
        );
        set_byte(&mut self.view, 0x38, 1);
        for i in 0..4 {
            let w = wheel(i);
            if f(&self.body, 0x1648) < 3. && f(&self.body, w + 0x104).abs() < 0.5 {
                let grip = f(&self.view, 0x5c + 4 * i);
                let grip = if grip < 10. { 10. } else { grip };
                let brake = f(&self.view, 0x28 + 4 * i);
                let cap = (f(&self.body, w + 0xe0) * grip) * 0.9;
                let value = if brake < 0. {
                    0.
                } else if brake <= cap {
                    brake
                } else {
                    cap
                };
                set(&mut self.view, 0x28 + 4 * i, value);
            }
            set_byte(&mut self.view, 0x39 + i, 0);
            set_byte(&mut self.view, 0x3d + i, 0);
            let spin = f(&self.body, w + 0x104);
            let state = if f(&self.view, 0x28 + 4 * i) == 0. || spin > EPSILON {
                1
            } else if spin < -EPSILON {
                2
            } else {
                0
            };
            integer(&mut self.view, 8 + 4 * i, state);
            set_byte(&mut self.view, 0x41 + i, 0);
            if f(&self.view, 0x5c + 4 * i) == 0. {
                integer(&mut self.view, 0x18 + 4 * i, 3);
                set(&mut self.view, 0x170 + 4 * i, 0.);
                set(&mut self.view, 0x180 + 4 * i, 0.);
            } else {
                integer(&mut self.view, 0x18 + 4 * i, 0);
            }
        }
    }
    fn torque_row(&mut self, system: &mut System, i: usize) -> Result<(), Error> {
        let w = wheel(i);
        let mut terms = vec![(self.id(0x128, i), f(&self.body, w + 0x100))];
        let driven = self.driven.contains(&i);
        if driven {
            terms.push((self.id(0x168, 0), -0. - f(&self.view, 0x7c + 4 * i)));
        }
        terms.push((self.id(0x118, i), f(&self.body, w + 0xe0)));
        terms.push((self.id(0x108, i), 1.));
        self.row(
            system,
            0xa0,
            i,
            &terms,
            if driven { f(&self.view, 0x2a8) } else { 0. },
        )
    }
    fn build(&mut self, delta: f32) -> Result<System, Error> {
        let mut system = System::reset_storage(self.solver);
        for i in 0..4 {
            for offset in [0x128, 0x138, 0x148] {
                integer(&mut self.view, offset + 4 * i, system.allocate_variable()?);
            }
        }
        for i in 0..4 {
            for offset in [0x118, 0x158] {
                integer(&mut self.view, offset + 4 * i, system.allocate_variable()?);
            }
            if self.id(0x18, i) == 0 {
                let rhs = dot(
                    vector(&self.view, 0x1d0 + 16 * i),
                    vector(&self.view, 0x190 + 16 * i),
                ) - f(&self.body, wheel(i) + 0x104) * f(&self.body, wheel(i) + 0xe0);
                self.row(
                    &mut system,
                    0xe0,
                    i,
                    &[
                        (self.id(0x128, i), f(&self.body, wheel(i) + 0xe0) * delta),
                        (self.id(0x138, i), -0. - delta),
                    ],
                    rhs,
                )?;
                let rhs = dot(
                    vector(&self.view, 0x210 + 16 * i),
                    vector(&self.view, 0x190 + 16 * i),
                );
                self.row(
                    &mut system,
                    0xf0,
                    i,
                    &[(self.id(0x148, i), -0. - delta)],
                    rhs,
                )?;
            } else {
                system.fix(self.id(0x118, i), f(&self.view, 0x180 + 4 * i))?;
                system.fix(self.id(0x158, i), f(&self.view, 0x170 + 4 * i))?;
            }
        }
        for i in 0..4 {
            integer(&mut self.view, 0x108 + 4 * i, system.allocate_variable()?);
            let state = self.id(8, i);
            if state == 0 {
                system.fix(
                    self.id(0x128, i),
                    (-1. / delta) * f(&self.body, wheel(i) + 0x104),
                )?;
            } else {
                let brake = f(&self.view, 0x28 + 4 * i);
                system.fix(
                    self.id(0x108, i),
                    if state == 1 { brake } else { -0. - brake },
                )?;
            }
        }
        integer(&mut self.view, 0x168, system.allocate_variable()?);
        system.fix(self.id(0x168, 0), f(&self.view, 0x58))?;
        integer(&mut self.view, 4, 1);
        for i in 0..4 {
            self.torque_row(&mut system, i)?;
            let inv = f(&self.body, 0x15a4);
            self.row(
                &mut system,
                0xb0,
                i,
                &[(self.id(0x138, i), -0.2), (self.id(0x118, i), inv)],
                0.,
            )?;
            self.row(
                &mut system,
                0xc0,
                i,
                &[(self.id(0x148, i), -0.2), (self.id(0x158, i), inv)],
                0.,
            )?;
        }
        Ok(system)
    }
    fn solve(&mut self, system: &mut System) -> Result<bool, Error> {
        for i in 0..4 {
            if self.id(0x18, i) == 0 {
                system.select(
                    &[self.id(0xf0, i), self.id(0xc0, i)],
                    &[self.id(0x158, i), self.id(0x148, i)],
                )?;
            } else {
                system.select(&[self.id(0xc0, i)], &[self.id(0x148, i)])?;
            }
            let spin = self.id(0x128, i);
            let spin = if system.is_fixed(spin)? {
                self.id(0x108, i)
            } else {
                spin
            };
            if self.id(0x18, i) == 0 {
                system.select(
                    &[self.id(0xe0, i), self.id(0xa0, i), self.id(0xb0, i)],
                    &[self.id(0x118, i), self.id(0x138, i), spin],
                )?;
            } else {
                system.select(
                    &[self.id(0xa0, i), self.id(0xb0, i)],
                    &[self.id(0x138, i), spin],
                )?;
            }
        }
        let success = system.remaining()?;
        set_byte(&mut self.view, 0x54, u8::from(success));
        Ok(success)
    }
    fn lateral_limit(&mut self, system: &mut System, i: usize) -> Result<(), Error> {
        let w = wheel(i);
        let spin = f(&self.body, w + 0x104);
        if spin <= 2. {
            return Ok(());
        }
        let blend = ((spin - 2.) * 0.1).min(1.);
        let speed = f(&self.body, w + 0xe0) * spin;
        let slip = dot(
            vector(&self.view, 0x190 + 16 * i),
            vector(&self.view, 0x210 + 16 * i),
        );
        let relative = (slip / speed).abs();
        let mass = f(&self.body, 0x15a0);
        let mut cap = ((mass * relative) * speed) * 0.7;
        if let Some(multiplier) = self.material_multipliers[i] {
            cap *= multiplier;
        }
        if f(&self.body, w + 0x78) > 0. {
            let front = f(&self.body, w + 8) > 0.;
            let base = if front { 0xff4 } else { 0x1004 };
            let bias = if front {
                f(&self.body, 0xfcc)
            } else {
                1. - f(&self.body, 0xfcc)
            };
            if speed != 0. {
                let inverse = f(&self.body, w + 0x78) / ((mass * speed) * 0.7);
                let ratio = f(&self.body, w + 0x138) / ((bias * mass) * f32::from_bits(0x409cf5c3));
                let mixture = (((ratio - 1.) * f(&self.body, 0x1014) + 1.)
                    * f(&self.body, base + 4))
                    * f(&self.body, base)
                    + (1. - f(&self.body, base)) * inverse;
                let mixture = mixture
                    .max(f(&self.body, base + 8))
                    .min(f(&self.body, base + 12));
                self.diagnostics[4 + i] = mixture;
                self.diagnostics[i] = if slip < 0. { -0. - relative } else { relative };
                cap /= mixture / inverse;
            }
        }
        let old = self.value(system, 0x158, i)?;
        let cap = f(&self.view, 0x6c + 4 * i) * cap;
        let limited = if cap < old.abs() {
            if old <= 0. { -0. - cap } else { cap }
        } else {
            old
        };
        system.set_value(self.id(0x158, i), (1. - blend) * old + limited * blend)
    }
    fn grip_limit(&mut self, system: &mut System, i: usize, ratio: f32) -> Result<(), Error> {
        let long = self.value(system, 0x118, i)?;
        let lat = self.value(system, 0x158, i)?;
        let w = wheel(i);
        let factor = if ratio <= 1. { 1. } else { 1. / ratio };
        system.fix(
            self.id(0x118, i),
            if ratio <= 1. { long } else { factor * long },
        )?;
        system.fix(
            self.id(0x158, i),
            if ratio <= 1. { lat } else { factor * lat },
        )?;
        set(&mut self.body, w + 0x9c, if ratio <= 1. { 0. } else { lat });
        set(
            &mut self.body,
            w + 0xa0,
            if ratio <= 1. { 0. } else { long },
        );
        let spill = if ratio <= 1. {
            0.
        } else {
            (ratio - 1.).clamp(0., 1.) * f(&self.view, 0x5c + 4 * i)
        };
        set(&mut self.body, w + 0xa4, spill);
        integer(&mut self.view, 0x18 + 4 * i, 3);
        system.remove(self.id(0xe0, i))?;
        system.remove(self.id(0xf0, i))
    }
    fn limit(&mut self, system: &mut System) -> Result<bool, Error> {
        let mut selected = None;
        let mut greatest = 1.;
        for i in 0..4 {
            if self.id(8, i) == 0 && byte(&self.view, 0x3d + i) == 0 {
                let ratio = (self.value(system, 0x108, i)? / f(&self.view, 0x28 + 4 * i)).abs();
                if greatest < ratio {
                    greatest = ratio;
                    selected = Some((false, i));
                }
            }
        }
        let mut ratios = [0.; 4];
        for (i, ratio) in ratios.iter_mut().enumerate() {
            if self.id(0x18, i) == 0 {
                self.lateral_limit(system, i)?;
                let long = self.value(system, 0x118, i)?;
                let lat = self.value(system, 0x158, i)?;
                let grip = f(&self.view, 0x5c + 4 * i);
                *ratio = (lat * lat + long * long) / (grip * grip);
                if greatest * greatest < *ratio {
                    greatest = ratio.sqrt();
                    selected = Some((true, i));
                }
            }
        }
        let Some((grip, i)) = selected else {
            return Ok(false);
        };
        if grip {
            self.grip_limit(system, i, greatest)?;
            if self.driven.contains(&i) {
                for (j, ratio) in ratios.into_iter().enumerate() {
                    if self.id(0x18, j) != 3 && self.driven.contains(&j) {
                        self.grip_limit(system, j, ratio.sqrt())?;
                    }
                }
            }
        } else {
            let old = self.value(system, 0x108, i)?;
            let brake = f(&self.view, 0x28 + 4 * i);
            system.fix(
                self.id(0x108, i),
                if old <= 0. { -0. - brake } else { brake },
            )?;
            integer(&mut self.view, 8 + 4 * i, if old <= 0. { 2 } else { 1 });
            system.unfix(self.id(0x128, i))?;
            set_byte(&mut self.view, 0x39 + i, 1);
            system.remove(self.id(0xa0, i))?;
            self.torque_row(system, i)?;
        }
        self.solve(system)?;
        Ok(true)
    }
    fn advance(&mut self, system: &System, delta: f32) -> Result<f32, Error> {
        let mut duration = delta;
        let mut event = None;
        let mut stops = [false; 4];
        for (i, stop) in stops.iter_mut().enumerate() {
            if self.id(8, i) != 0
                && byte(&self.view, 0x39 + i) == 0
                && self.value(system, 0x108, i)? != 0.
            {
                let acceleration = self.value(system, 0x128, i)?;
                if acceleration != 0. {
                    let time = (-0. - f(&self.body, wheel(i) + 0x104)) / acceleration;
                    if time > 0. && time < delta {
                        *stop = true;
                        if time < duration {
                            duration = time;
                            event = Some(i);
                        }
                    }
                }
            }
        }
        if duration >= EPSILON {
            for i in 0..4 {
                let brake = self.value(system, 0x108, i)?;
                let spin =
                    self.value(system, 0x128, i)? * duration + f(&self.body, wheel(i) + 0x104);
                set(&mut self.body, wheel(i) + 0x118, brake);
                set(&mut self.body, wheel(i) + 0x104, spin);
            }
            let body = dynamics::ImpulseBody {
                mass: f(&self.body, 0x15a0),
                inertia: std::array::from_fn(|i| f(&self.body, 0x15a8 + 4 * i)),
                basis: std::array::from_fn(|i| vector(&self.body, 0x15e0 + 16 * i)),
            };
            for i in 0..4 {
                let long = self.value(system, 0x118, i)?;
                let lat = self.value(system, 0x158, i)?;
                set(&mut self.body, wheel(i) + 0xb0, lat);
                set(&mut self.body, wheel(i) + 0xb4, long);
                let long_scale = long * duration;
                let lat_scale = lat * duration;
                let impulse = std::array::from_fn(|j| {
                    lat_scale * f(&self.view, 0x210 + 16 * i + 4 * j)
                        + long_scale * f(&self.view, 0x1d0 + 16 * i + 4 * j)
                });
                let (linear, angular) = dynamics::apply_impulse(
                    body,
                    vector(&self.body, 0x15c0),
                    vector(&self.body, 0x15d0),
                    impulse,
                    vector(&self.view, 0x250 + 16 * i),
                )?;
                for j in 0..4 {
                    set(&mut self.body, 0x15c0 + 4 * j, linear[j]);
                    set(&mut self.body, 0x15d0 + 4 * j, angular[j]);
                }
            }
        }
        if let Some(i) = event {
            integer(&mut self.view, 8 + 4 * i, 0);
            set_byte(&mut self.view, 0x3d + i, 1);
            set(&mut self.body, wheel(i) + 0x104, 0.);
            for (j, stop) in stops.into_iter().enumerate() {
                if stop && byte(&self.view, 0x3d + j) == 0 {
                    set_byte(&mut self.view, 0x3d + j, 1);
                    set(&mut self.body, wheel(j) + 0x104, 0.);
                    integer(&mut self.view, 8 + 4 * j, 0);
                }
            }
            Ok(delta - duration)
        } else {
            Ok(0.)
        }
    }
    #[allow(clippy::assign_op_pattern)] // Retain the observed add operand order.
    fn run(&mut self, delta: f32) -> Result<(), Error> {
        self.initialize();
        let extra = f(&self.view, 0x294) / self.driven.len() as f32;
        let saved: Vec<_> = self
            .driven
            .iter()
            .map(|i| f(&self.body, wheel(*i) + 0x100))
            .collect();
        for (&i, &old) in self.driven.iter().zip(&saved) {
            set(&mut self.body, wheel(i) + 0x100, extra + old);
        }
        let mut remaining = delta;
        let mut events = 0;
        while remaining > 0. {
            events += 1;
            if events > 64 {
                return Err(Error("vehicle force event bound exceeded"));
            }
            let mut system = self.build(remaining)?;
            self.solve(&mut system)?;
            let mut limits = 0;
            while byte(&self.view, 0x54) != 0 && self.limit(&mut system)? {
                limits += 1;
                if limits > 64 {
                    return Err(Error("vehicle force limit bound exceeded"));
                }
            }
            for i in 0..4 {
                let lat = self.value(&system, 0x158, i)?;
                let long = self.value(&system, 0x118, i)?;
                set(&mut self.body, wheel(i) + 0x84, lat);
                set(&mut self.body, wheel(i) + 0x88, long);
                for (dest, source) in [(0x84, 0x9c), (0x88, 0xa0)] {
                    let value = f(&self.body, wheel(i) + source);
                    if value != 0. {
                        set(&mut self.body, wheel(i) + dest, value);
                    }
                }
            }
            if byte(&self.view, 0x54) == 0 {
                for i in 0..4 {
                    for offset in [0x118, 0x128, 0x138, 0x158] {
                        system.fix(self.id(offset, i), 0.)?;
                    }
                }
            }
            remaining = self.advance(&system, remaining)?;
            for i in 0..4 {
                if self.id(0x18, i) == 3 {
                    let long = self.value(&system, 0x118, i)?;
                    let lat = self.value(&system, 0x158, i)?;
                    set(&mut self.view, 0x180 + 4 * i, long);
                    set(&mut self.view, 0x170 + 4 * i, lat);
                }
            }
            self.solver = system.snapshot();
        }
        let mut sum = 0.;
        for (&i, &old) in self.driven.iter().zip(&saved) {
            sum = f(&self.body, wheel(i) + 0x104) + sum;
            set(&mut self.body, wheel(i) + 0x100, old);
        }
        let ratio = f(&self.body, 0x18f4 + 4 * u(&self.body, 0x196c));
        let ratio = if ratio == 0. { 1. } else { ratio };
        let mut speed = (sum / self.driven.len() as f32) / ratio;
        let old = f(&self.body, 0x1894);
        let decay = 1. - delta * 2.;
        let mut minimum = old * decay;
        if old < f(&self.body, 0x17a8) * 0.98 {
            minimum = 0.;
        }
        let maximum = f(&self.body, 0x17a4);
        if maximum < minimum {
            minimum = maximum * decay;
        }
        let ceiling = (maximum * delta) * 2000. + old;
        if byte(&self.body, 0x1971) == 0 {
            minimum = 0.;
        }
        if minimum <= speed {
            if ceiling < speed {
                speed = ceiling;
            }
        } else {
            speed = minimum;
        }
        set_byte(&mut self.body, 0x1f0c, 1);
        set(&mut self.body, 0x1894, speed);
        Ok(())
    }
}

/// All validation and execution happen on a copy; errors leave the caller intact.
pub fn apply(frame: &mut Frame, delta: f32) -> Result<(), Error> {
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || byte(&frame.body, 0xadc) != 0
        || u(&frame.body, 0x1598) != 1
        || u(&frame.body, 0x1620) != 4
        || u(&frame.body, 0x196c) >= 16
        || frame.driven.is_empty()
        || frame.driven.len() > 4
        || frame
            .driven
            .iter()
            .enumerate()
            .any(|(i, v)| *v >= 4 || frame.driven[..i].contains(v))
        || frame
            .material_multipliers
            .iter()
            .flatten()
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid selected vehicle force frame"));
    }
    let gear = u(&frame.body, 0x196c);
    let body_scalars = [
        0x15a0,
        0x15a4,
        0x15a8,
        0x15ac,
        0x15b0,
        0x17a0,
        0x17a4,
        0x17a8,
        0x1894,
        0x18f4 + 4 * gear,
        0x1648,
        0xe34,
        0xfcc,
        0xff4,
        0xff8,
        0xffc,
        0x1000,
        0x1004,
        0x1008,
        0x100c,
        0x1010,
        0x1014,
    ];
    if body_scalars
        .into_iter()
        .any(|offset| !f(&frame.body, offset).is_finite())
        || (0x15c0..0x1620)
            .step_by(4)
            .any(|offset| !f(&frame.body, offset).is_finite())
        || [0x58, 0x9c, 0x2a8]
            .into_iter()
            .any(|offset| !f(&frame.view, offset).is_finite())
        || (0..4).any(|i| {
            [0x104, 0x100, 0xe0, 0x78, 8, 0x138, 0x9c, 0xa0, 0xa4]
                .into_iter()
                .any(|offset| !f(&frame.body, wheel(i) + offset).is_finite())
                || [0x28, 0x5c, 0x6c, 0x7c]
                    .into_iter()
                    .any(|offset| !f(&frame.view, offset + 4 * i).is_finite())
        })
        || (0x190..0x290)
            .step_by(4)
            .any(|offset| !f(&frame.view, offset).is_finite())
        || [0x15a0, 0x15a8, 0x15ac, 0x15b0]
            .into_iter()
            .any(|offset| f(&frame.body, offset) <= 0.)
        || (0..4).any(|i| {
            f(&frame.body, wheel(i) + 0x100) <= 0. || f(&frame.body, wheel(i) + 0xe0) <= 0.
        })
    {
        return Err(Error("nonfinite or invalid physical force inputs"));
    }
    let mut next = frame.clone();
    next.run(delta)?;
    *frame = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_frame_leaves_all_storage_intact() {
        let mut frame = Frame {
            body: [0; BODY_WORDS],
            view: [0; VIEW_WORDS],
            solver: [0xdeadbeef; 884],
            driven: vec![0],
            material_multipliers: [None; 4],
            diagnostics: [0.; 8],
        };
        integer(&mut frame.body, 0x1598, 1);
        integer(&mut frame.body, 0x1620, 4);
        let before = frame.clone();
        assert!(apply(&mut frame, 0.016).is_err());
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
    }
}
