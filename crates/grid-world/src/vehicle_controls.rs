//! Selected whole control wrapper followed by the verified propulsion schedule.
//! Gear-search flag +1348 and the external reverse-disable global are inactive.
use crate::{
    Error, assisted_steering, steering_coefficients, steering_input, vehicle_force::Frame,
    vehicle_propulsion,
};
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, x: f32) {
    b[o / 4] = x.to_bits();
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> ((o % 4) * 8)) as u8
}
fn put_byte(b: &mut [u32], o: usize, x: u8) {
    let s = (o % 4) * 8;
    b[o / 4] = (b[o / 4] & !(255 << s)) | ((x as u32) << s);
}
fn w(i: usize) -> usize {
    0x1990 + i * 0x150
}
fn c(x: u32) -> f32 {
    f32::from_bits(x)
}
fn draw(b: &mut [u32]) -> u32 {
    let x = b[0x1644 / 4]
        .wrapping_mul(0x41c64e6d)
        .wrapping_add(0x40584c0f);
    b[0x1644 / 4] = x;
    x % 65535
}
fn event(b: &mut [u32], start: usize, count: usize, value: u32) {
    let n = b[count / 4] as usize;
    if n < 4 {
        b[start / 4 + n] = value;
        b[count / 4] += 1;
    }
}
fn projection(b: &[u32]) -> f32 {
    (f(b, 0x15c4) * f(b, 0x1604) + f(b, 0x15c0) * f(b, 0x1600))
        + (f(b, 0x15cc) * f(b, 0x160c) + f(b, 0x15c8) * f(b, 0x1608))
}

fn steering(s: &mut Frame, pair: [usize; 2], delta: f32) -> Result<(), Error> {
    let b = &s.body;
    let input = steering_coefficients::Input {
        command: steering_input::Input {
            command: f(b, 0xe30),
            perturbation: f(b, 0x2294),
            speed_scalar: f(b, 0x1648),
            phase_scalar: f(b, 0x159c),
        },
        scale_e54: f(b, 0xe54),
        span_21d4: f(b, 0x21d4),
        span_21d8: f(b, 0x21d8),
        cap_2298: f(b, 0x2298),
        cap_229c: f(b, 0x229c),
        mode_1f74_is_one: b[0x1f74 / 4] == 1,
        delta,
        pair,
        wheels: std::array::from_fn(|i| steering_coefficients::Wheel {
            x: f(b, w(i)),
            toe_e8: f(b, w(i) + 0xe8),
            rotation_offset: f(b, 0x22a0 + i * 4),
            frozen: byte(b, 0x22cc + i) != 0,
            blend_20: f(b, w(i) + 0x20),
            blend_24: f(b, w(i) + 0x24),
            smooth_28: f(b, w(i) + 0x28),
            smooth_2c: f(b, w(i) + 0x2c),
            limit_f0: f(b, w(i) + 0xf0),
            rotation_144: f(b, w(i) + 0x144),
        }),
    };
    let states = if byte(b, 0xe6c) == 0 {
        steering_coefficients::unassisted(input)?
    } else {
        let p = assisted_steering::Parameters {
            command_deadzone_fdc: f(b, 0xfdc),
            shaping_138c: f(b, 0x138c),
            high_scale_123c: f(b, 0x123c),
            scale_start_1240: f(b, 0x1240),
            scale_end_1244: f(b, 0x1244),
            speed_cap_e70: f(b, 0xe70),
            minimum_scale_e5c: f(b, 0xe5c),
            feedback_gain_e58: f(b, 0xe58),
            low_gain_101c: f(b, 0x101c),
            gain_speed_1020: f(b, 0x1020),
            maximum_angle_1024: f(b, 0x1024),
            minimum_angle_1028: f(b, 0x1028),
            feedback_deadzone_fec: f(b, 0xfec),
            linear_response_fe8: f(b, 0xfe8),
            slew_low_1248: f(b, 0x1248),
            slew_high_1250: f(b, 0x1250),
            slew_start_124c: f(b, 0x124c),
            slew_end_1254: f(b, 0x1254),
            input_factor_1388: f(b, 0x1388),
            smoothing_factor_1f2c: f(b, 0x1f2c),
        };
        let motion = assisted_steering::Motion {
            matrix: std::array::from_fn(|i| std::array::from_fn(|j| f(b, 0x15e0 + i * 16 + j * 4))),
            linear: std::array::from_fn(|i| f(b, 0x15c0 + i * 4)),
            angular: std::array::from_fn(|i| f(b, 0x15d0 + i * 4)),
            wheel_points_2_3: std::array::from_fn(|i| {
                std::array::from_fn(|j| f(b, w(i + 2) + j * 4))
            }),
        };
        assisted_steering::calculate(input, p, motion, std::array::from_fn(|i| f(b, w(i) + 0x38)))?
    };
    for (i, state) in states.into_iter().enumerate() {
        for (o, x) in [
            (0x20, state.blend_20),
            (0x24, state.blend_24),
            (0x28, state.smooth_28),
            (0x2c, state.smooth_2c),
            (0x38, state.history_38),
        ] {
            set(&mut s.body, w(i) + o, x);
        }
    }
    Ok(())
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed value-first sums"
)]
fn automatic(b: &mut [u32]) {
    let mut scale = 1_f32;
    let mut slip = 0_f32;
    let mut count = 0;
    for i in 0..4 {
        if byte(b, 0x227c + i) != 0 && byte(b, 0x2278 + i) == 0 && f(b, w(i) + 0x10c) > 0. {
            let x = f(b, w(i) + 0xf8) / f(b, w(i) + 0xf4);
            if x <= scale {
                scale = x;
            }
        }
        if f(b, w(i) + 0x10c) > 0. && f(b, w(i) + 0x78) > 0. {
            slip = f(b, w(i) + 0xa0) / f(b, w(i) + 0x78) + slip;
            count += 1;
        }
    }
    if count != 0 {
        slip /= count as f32;
    }
    let gear = b[0x196c / 4] as usize;
    if gear != 0 && gear != 9 {
        let speed = projection(b).max(1.);
        if f(b, 0x1974) > c(0x3e4ccccd) {
            let mut candidate = gear;
            if f(b, 0x18f4 + gear * 4) > 0.
                && f(b, 0x18f8 + gear * 4) <= 0.
                && f(b, 0x2280) > c(0x3f4ccccd)
            {
                candidate -= 1;
            }
            if f(b, 0x1944 + candidate * 4) * scale <= speed {
                if speed > f(b, 0x191c + gear * 4) * scale
                    || (f(b, 0x1894) / f(b, 0x17a4) > c(0x412f3333) && slip > 10.)
                {
                    put_byte(b, 0xe24, 1);
                    set(b, 0x1974, 0.);
                }
            } else if slip < 1. {
                put_byte(b, 0xe28, 1);
                set(b, 0x1974, 0.);
            }
        }
    }
    if byte(b, 0x1972) != 0 {
        if b[0x196c / 4] == 0 {
            b[0x196c / 4] = 1;
        }
        if b[0x196c / 4] == 9 {
            b[0xe1c / 4] = b[0xe40 / 4];
            b[0x185c / 4] = b[0xe1c / 4];
            b[0xe20 / 4] = b[0xe3c / 4];
        }
        if f(b, 0x1648) < 3. && f(b, 0xe3c) + c(0x3d4ccccd) < f(b, 0xe40) && byte(b, 0x21f2) == 0 {
            b[0x196c / 4] = 9;
            put_byte(b, 0x198d, 0);
        }
        if f(b, 0x1648) < f(b, 0x1378)
            && (b[0x196c / 4] == 0 || b[0x196c / 4] == 9)
            && f(b, 0xe40) + c(0x3d4ccccd) < f(b, 0xe3c)
        {
            b[0x196c / 4] = 1;
            put_byte(b, 0x198d, 0);
        }
    }
}

fn change_gear(b: &mut [u32], old: usize, next: usize) -> Result<(), Error> {
    let mut emitted = false;
    set(b, 0x1974, 0.);
    if byte(b, 0x1971) != 0 || f(b, 0xe34) == 0. {
        if next != 0 && old != 0 && byte(b, 0x1640) == 0 && f(b, 0x226c) == 0. {
            if f(b, 0x18f4 + next * 4) == 0. {
                return Err(Error("selected gear transition divides by zero"));
            }
            let x = (f(b, 0x18f4 + old * 4) / f(b, 0x18f4 + next * 4)) * f(b, 0x1894);
            set(b, 0x1894, x);
        }
        if byte(b, 0x1971) == 0 && next != 0 {
            event(b, 0x21dc, 0x21ec, 0);
            emitted = true;
        }
    }
    let speed = f(b, 0x1648);
    if (b[0x196c / 4] as usize) < next && b[0x196c / 4] != 9 && next != 1 {
        let threshold = if f(b, 0x22b0) > 0.5 {
            (f(b, 0x22b0) - 0.5) * 2.
        } else {
            0.
        };
        if ((draw(b) & 255) as f32) < threshold * 256. {
            if !emitted {
                event(b, 0x21dc, 0x21ec, 0);
            }
            b[0x196c / 4] = next as u32;
            return Ok(());
        }
        if speed >= 2. {
            event(b, 0x18b0, 0x18c0, 1);
        }
    }
    if !emitted && next != 0 && (speed < 2. || (next as u32) < b[0x196c / 4]) {
        event(b, 0x21dc, 0x21ec, 1);
    }
    b[0x196c / 4] = next as u32;
    Ok(())
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed value-first sum"
)]
fn rev_match(s: &mut Frame) {
    let b = &mut s.body;
    set(b, 0xe34, 1.);
    let n = s.driven.len() as f32;
    let mut mean = 0.;
    for &i in &s.driven {
        mean = f(b, w(i) + 0x104) * (1. / n) + mean;
    }
    mean /= f(b, 0x18f4 + b[0x196c / 4] as usize * 4);
    let mode = b[0x1858 / 4];
    let stop = if mean - f(b, 0x1894) <= 0. {
        set(b, 0x185c, 0.);
        put_byte(b, 0x1861, 1);
        if mode == 2 {
            true
        } else {
            if mode != 1 {
                b[0x1858 / 4] = 1;
            }
            mean < f(b, 0x17a8)
        }
    } else {
        set(b, 0x185c, 1.);
        if mode == 1 {
            true
        } else {
            if mode != 2 {
                b[0x1858 / 4] = 2;
            }
            mean < f(b, 0x17a8)
        }
    };
    if stop {
        set(b, 0xe34, 0.);
        put_byte(b, 0x18c8, 0);
    }
}

fn brakes(s: &mut Frame, delta: f32) {
    let (b, v) = (&mut s.body, &mut s.view);
    let mode = b[0x11bc / 4];
    b[0x197c / 4] = mode;
    let magnitude = (f(b, 0x15c0) * f(b, 0x15c0) + f(b, 0x15c4) * f(b, 0x15c4))
        + (f(b, 0x15c8) * f(b, 0x15c8) + f(b, 0x15cc) * f(b, 0x15cc));
    let active = (mode == 2 || (mode == 1 && f(b, 0x1980) < 1.25))
        && magnitude >= 0.25
        && byte(b, 0x1640) == 0;
    let mut exceeded = false;
    for i in 0..4 {
        let command = if byte(b, 0x1559) != 0 {
            (f(b, 0xe20) * c(0x3f8e38e4)).clamp(0., 1.)
        } else {
            f(b, 0xe20)
        };
        let hand = if byte(b, 0x1640) != 0 && byte(b, w(i) + 0xfc) != 0 {
            ((f(b, 0x1260) / (f(b, 0x125c) + f(b, 0x1260))) * (f(b, 0x1d38) + f(b, 0x1a98)))
                * f(b, 0x1358)
        } else {
            0.
        };
        let mut force = command * f(b, w(i) + 0x108) + hand;
        force *= f(b, 0x2284 + i * 4);
        force *= f(b, 0x1ef4);
        let limit = if f(b, w(i) + 0xa4) <= 0. {
            (f(v, 0x5c + i * 4) * c(0x3f733333)) * f(b, w(i) + 0xe0)
        } else {
            0.
        };
        if limit < force {
            exceeded = true;
            if active {
                force = limit;
            }
        }
        set(v, 0x28 + i * 4, force);
    }
    if exceeded {
        let x = f(b, 0x1980) + delta;
        set(b, 0x1980, x);
        put_byte(b, 0x1984, u8::from(active));
    } else {
        let x = (f(b, 0x1980) - delta).clamp(0., f32::MAX);
        set(b, 0x1980, x);
        put_byte(b, 0x1984, 0);
    }
    for i in 0..4 {
        let x = f(v, 0x28 + i * 4).max(f(v, 0x8c + i * 4));
        set(v, 0x28 + i * 4, x);
    }
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed arithmetic order"
)]
fn control(s: &mut Frame, pair: [usize; 2], delta: f32) -> Result<(), Error> {
    if s.body[0x196c / 4] == 9 && byte(&s.body, 0x1972) != 0 {
        s.body.swap(0xe1c / 4, 0xe20 / 4);
    }
    // Missing driven mechanics only changes mode 0, excluded by selected force mode 1.
    steering(s, pair, delta)?;
    s.body[0x185c / 4] = s.body[0xe1c / 4];
    let old = s.body[0x196c / 4] as usize;
    let clutch = byte(&s.body, 0xe18);
    put_byte(&mut s.body, 0x1860, clutch);
    if byte(&s.body, 0x1971) != 0 {
        set(&mut s.body, 0xe34, 0.);
        if byte(&s.body, 0x1640) != 0 {
            set(&mut s.body, 0xe34, 1.);
        }
    }
    if f(&s.body, 0x226c) != 0. {
        put_byte(&mut s.body, 0xe18, 0);
        set(&mut s.body, 0xe34, 0.);
        for o in [0xe20, 0xe40] {
            set(&mut s.body, o, 1.);
        }
        for o in [0xe1c, 0xe3c] {
            set(&mut s.body, o, 0.);
        }
        s.body[0x196c / 4] = 1;
    }
    put_byte(&mut s.body, 0x1861, 0);
    if byte(&s.body, 0x18c8) != 0 && byte(&s.body, 0x1971) != 0 {
        rev_match(s);
    }
    let b = &mut s.body;
    if f(b, 0x1894) < f(b, 0x17a8) && f(b, 0x1fa0) > 0. && f(b, 0x185c) > 0.5 {
        put_byte(b, 0x1860, 1);
    }
    let timer = f(b, 0x1974) + delta;
    set(b, 0x1974, timer);
    if byte(b, 0x1970) != 0 {
        put_byte(b, 0xe28, 0);
        put_byte(b, 0xe24, 0);
        b[0xe2c / 4] = 0;
        automatic(b);
    }
    let multiple = f(b, 0x2280) > c(0x3f4ccccd);
    let mut next = old;
    if multiple && old != 9 && f(b, 0x18f8 + old * 4) > 0. && f(b, 0x18fc + old * 4) <= 0. {
        next = old + 1;
    }
    if byte(b, 0xe28) != 0 {
        put_byte(b, 0xe28, 0);
        next = if old == 0 {
            9
        } else if old != 9 {
            old - 1
        } else {
            next
        };
        if multiple && next != 9 && f(b, 0x18f8 + next * 4) > 0. && f(b, 0x18fc + next * 4) <= 0. {
            next = next
                .checked_sub(1)
                .ok_or(Error("gear selection underflows supported table"))?;
        }
    }
    if byte(b, 0xe24) != 0 {
        put_byte(b, 0xe24, 0);
        if old == 9 {
            next = 0;
        } else if f(b, 0x18f8 + old * 4) > 0. {
            next = old + 1;
            if old != 0 {
                set(b, 0x2e4, 0.);
            }
        }
        if multiple && next != 9 && f(b, 0x18f8 + next * 4) > 0. && f(b, 0x18fc + next * 4) <= 0. {
            next += 1;
        }
    }
    let override_gear = b[0xe48 / 4] as i32;
    if override_gear != 10 {
        b[0xe48 / 4] = 10;
        next = override_gear as usize;
    }
    let request = b[0xe2c / 4] as i32;
    if request > 0 && f(b, 0x18f4 + request as usize * 4) != 0. {
        next = request as usize;
    }
    if request < 0 {
        next = 9;
    }
    b[0xe2c / 4] = 0;
    if next > 9 {
        return Err(Error("gear selection exceeds supported table"));
    }
    if next != old {
        let flag = u8::from(byte(b, 0x1970) != 0 && byte(b, 0x1972) == 0);
        put_byte(b, 0x198d, flag);
        change_gear(b, old, next)?;
    }
    brakes(s, delta);
    let b = &mut s.body;
    if f(b, 0x226c) != 0. {
        put_byte(b, 0xe18, 0);
        put_byte(b, 0x1860, 0);
        b[0x196c / 4] = 1;
        set(b, 0x185c, 0.);
    }
    if f(b, 0x1978) > 0. && byte(b, 0x1971) != 0 {
        let x = (f(b, 0x1978) - delta).clamp(0., 999.);
        set(b, 0x1978, x);
        set(b, 0xe34, 1.);
    }
    Ok(())
}

/// Body +21CC/21D0 identities are supplied as owned wheel ordinals.
pub fn apply(
    frame: &mut Frame,
    pair: [usize; 2],
    roll: [Option<f32>; 4],
    delta: f32,
) -> Result<(), Error> {
    let b = &frame.body;
    let scalars = [
        0xe1c, 0xe20, 0xe34, 0xe3c, 0xe40, 0x1378, 0x1894, 0x17a4, 0x17a8, 0x185c, 0x1974, 0x1978,
        0x1980, 0x226c, 0x2280, 0x22b0, 0x125c, 0x1260, 0x1358, 0x1ef4,
    ];
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || !matches!(b[0x1598 / 4], 1 | 2)
        || b[0x1620 / 4] != 4
        || byte(b, 0x1348) != 0
        || b[0x196c / 4] > 9
        || b[0x21ec / 4] > 4
        || b[0x18c0 / 4] > 4
        || !(-1..=9).contains(&(b[0xe2c / 4] as i32))
        || b[0xe48 / 4] > 10
        || frame.driven.is_empty()
        || frame.driven.len() > 4
        || frame
            .driven
            .iter()
            .enumerate()
            .any(|(i, &x)| x >= 4 || frame.driven[..i].contains(&x))
        || scalars.into_iter().any(|o| !f(b, o).is_finite())
        || f(b, 0x17a4) <= 0.
        || f(b, 0x125c) + f(b, 0x1260) == 0.
        || (0..12).any(|i| !f(b, 0x18f4 + i * 4).is_finite())
        || (0..10).any(|i| !f(b, 0x191c + i * 4).is_finite() || !f(b, 0x1944 + i * 4).is_finite())
        || (0..4).any(|i| {
            [0x78, 0xa0, 0xa4, 0xe0, 0xf4, 0xf8, 0x104, 0x108, 0x10c]
                .into_iter()
                .any(|o| !f(b, w(i) + o).is_finite())
                || f(b, w(i) + 0xf4) == 0.
                || !f(b, 0x2284 + i * 4).is_finite()
        })
    {
        return Err(Error("invalid selected control input"));
    }
    let mut s = frame.clone();
    control(&mut s, pair, delta)?;
    vehicle_propulsion::apply(&mut s, roll, delta)?;
    *frame = s;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn failure_frame() -> Frame {
        let mut frame = Frame {
            body: [0; crate::vehicle_force::BODY_WORDS],
            view: [0; crate::vehicle_force::VIEW_WORDS],
            solver: [0xdeadbeef; 884],
            driven: vec![2, 3],
            material_multipliers: [None; 4],
            diagnostics: [1.; 8],
        };
        frame.body[0x1598 / 4] = 1;
        frame.body[0x1620 / 4] = 4;
        frame.body[0x196c / 4] = 1;
        frame.body[0xe48 / 4] = 10;
        frame.body[0xe2c / 4] = 2; // Force a gear event before the later failure.
        for (o, x) in [
            (0x17a0, 0.22),
            (0x17a4, 900.),
            (0x17a8, 105.),
            (0x1f5c, 1.),
            (0x1f98, 3600.),
            (0x1fa0, 3600.),
            (0x125c, 1.),
            (0x1260, 1.),
            (0xe54, 0.4),
            (0x21d4, 1.6),
            (0x21d8, 2.6),
            (0x2298, 1.),
            (0x229c, 1.),
            (0xe30, 0.2),
        ] {
            set(&mut frame.body, o, x);
        }
        for i in 1..=9 {
            set(&mut frame.body, 0x18f4 + i * 4, 0.5);
        }
        for i in 0..4 {
            set(&mut frame.body, w(i) + 0xf0, 1.);
            set(&mut frame.body, w(i) + 0xf4, 1.);
        }
        frame
    }
    #[test]
    fn selected_zero_ratio_rejects_after_steering_without_publishing_changes() {
        let mut frame = failure_frame();
        frame.body[0xe2c / 4] = 0;
        frame.body[0xe48 / 4] = 7;
        set(&mut frame.body, 0x18f4 + 7 * 4, 0.);
        let before = frame.clone();
        let error = apply(&mut frame, [3, 2], [None; 4], 0.01).unwrap_err();
        assert_eq!(error.0, "selected gear transition divides by zero");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
    #[test]
    fn single_forward_gear_downshift_rejects_underflow_without_panicking() {
        let mut frame = failure_frame();
        frame.body[0xe2c / 4] = 0;
        put_byte(&mut frame.body, 0xe28, 1);
        set(&mut frame.body, 0x2280, 1.);
        for i in 2..=8 {
            set(&mut frame.body, 0x18f4 + i * 4, 0.);
        }
        let before = frame.clone();
        let error = apply(&mut frame, [3, 2], [None; 4], 0.01).unwrap_err();
        assert_eq!(error.0, "gear selection underflows supported table");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
    }
    #[test]
    fn later_force_failure_preserves_controls_gears_and_all_force_storage() {
        let mut frame = failure_frame();
        // Unused sparse slots must not prevent the earlier valid shift to gear two.
        for i in 7..=8 {
            set(&mut frame.body, 0x18f4 + i * 4, 0.);
        }
        let before = frame.clone();
        let error = apply(&mut frame, [3, 2], [None; 4], 0.01).unwrap_err();
        assert_eq!(error.0, "nonfinite or invalid physical force inputs");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
}
