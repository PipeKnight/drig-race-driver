//! Joined post-control engine, differential, traction and selected force segment.
//! Control/steering, contact geometry and timing have already been supplied.
use crate::{
    Error,
    vehicle_force::{self, Frame},
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
fn set_byte(b: &mut [u32], o: usize, x: u8) {
    let shift = (o % 4) * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | ((x as u32) << shift);
}
fn wheel(i: usize) -> usize {
    0x1990 + i * 0x150
}
fn draw(b: &mut [u32]) -> u32 {
    let x = b[0x1644 / 4]
        .wrapping_mul(0x41c64e6d)
        .wrapping_add(0x40584c0f);
    b[0x1644 / 4] = x;
    x % 65535
}
fn c(bits: u32) -> f32 {
    f32::from_bits(bits)
}

pub(crate) fn curve53(b: &[u32], mut speed: f32) -> f64 {
    if speed < 0. {
        speed = 0.;
    } else if speed > c(0x44d17084) {
        return 0.;
    }
    let scaled = speed * c(0x3c979102);
    let raw = scaled as usize;
    let i = raw.min(30);
    let a = f(b, 0x17ac + i * 4);
    if a == 0. {
        return 0.;
    }
    // SSE stores the fractional index; x87 interpolates in 53-bit precision.
    ((f(b, 0x17b0 + i * 4) as f64 - a as f64) * (scaled - raw as f32) as f64) + a as f64
}

fn curve(b: &[u32], speed: f32) -> f32 {
    // The engine caller stores the extended return before torque math.
    curve53(b, speed) as f32
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed factor-first arithmetic"
)]
fn engine(b: &mut [u32], delta: f32) -> Result<f32, Error> {
    let e = 0x17a0;
    let damping = 1. - delta * 4.;
    set(
        b,
        e + 0x100,
        f(b, e + 0x100) * damping - (f(b, e + 0xfc) * delta) * 9.,
    );
    if (draw(b) & 511) as i32 <= ((delta * 15.) * 512.) as i32 {
        let random = (draw(b) & 255) as f32;
        set(
            b,
            e + 0x100,
            (random - 127.5) * c(0x3a9a33ce) + f(b, e + 0x100),
        );
    }
    set(b, e + 0xfc, f(b, e + 0x100) * delta + f(b, e + 0xfc));
    if f(b, e + 0xfc) > c(0x3b75c290) {
        set(b, e + 0xfc, c(0x3b75c290));
        set(b, e + 0x100, f(b, e + 0x100).abs() * c(0xbf4ccccd));
    }
    if f(b, e + 0xfc) < c(0xbb75c290) {
        set(b, e + 0xfc, c(0xbb75c290));
        set(b, e + 0x100, f(b, e + 0x100).abs() * c(0x3f4ccccd));
    }
    if f(b, e + 0xf4) < f(b, e + 8) * c(0x3f7d70a4) {
        set(b, e + 0xfc, 0.);
        set(b, e + 0x100, 0.);
    }
    set(b, e + 0x108, f(b, e + 0x10c) * delta + f(b, e + 0x108));
    if f(b, e + 0x108) > c(0x3b75c290) {
        set(b, e + 0x108, c(0x3b75c290));
        set(b, e + 0x10c, f(b, e + 0x10c).abs() * c(0xbf4ccccd));
    }
    if f(b, e + 0x108) < c(0xbb75c290) {
        set(b, e + 0x108, c(0xbb75c290));
        set(b, e + 0x10c, f(b, e + 0x10c).abs() * c(0x3f4ccccd));
    }
    set(
        b,
        e + 0x10c,
        damping * f(b, e + 0x10c) - (f(b, e + 0x108) * delta) * 9.,
    );
    if (draw(b) & 511) as i32 <= ((delta * 25.) * 512.) as i32 {
        let random = (draw(b) & 255) as f32;
        set(
            b,
            e + 0x10c,
            (random - 127.5) * c(0x3a9a33ce) + f(b, e + 0x10c),
        );
    }
    if f(b, e + 0xf4) < f(b, e + 8) * c(0x3f7d70a4) {
        set(b, e + 0x108, 0.);
        set(b, e + 0x10c, 0.);
    }
    if f(b, e + 0xc8) > 0. {
        set(b, e + 0xc8, f(b, e + 0xc8) - delta);
    }
    if f(b, 0x22c4) > 0. {
        let threshold = ((f(b, 0x22c4) * delta) * c(0x400ccccd)) * 1024.;
        if ((draw(b) & 1023) as f32) < threshold {
            let time = (draw(b) & 255) as f32 * c(0x3976b948) + c(0x3ca3d70a);
            if f(b, e + 0xc8) < time {
                set(b, e + 0xc8, time);
            }
        }
    }
    let mut blend = f(b, e + 0xbc);
    if byte(b, 0x155a) != 0 {
        blend = (blend * c(0x3f8e38e4)).clamp(0., 1.);
    }
    if f(b, e + 0xc8) > 0. {
        blend = 0.;
    }
    let speed = f(b, e + 0xf4);
    let idle = f(b, e + 8);
    if speed <= f(b, e + 4) {
        set(b, e + 0xdc, delta + f(b, e + 0xdc));
    } else {
        set(b, e + 0xdc, 0.);
        blend = 0.;
    }
    let a: [f32; 4] = std::array::from_fn(|i| f(b, 0x15c0 + i * 4));
    let axis: [f32; 4] = std::array::from_fn(|i| f(b, 0x1600 + i * 4));
    let projection = (a[1] * axis[1] + a[0] * axis[0]) + (a[3] * axis[3] + a[2] * axis[2]);
    let base = if projection < 0. && b[0x196c / 4] == 1 {
        f(b, e + 0xd0)
    } else {
        curve(b, speed)
    };
    let mut base = ((((f(b, 0x22c8) * base) * f(b, e + 0xe8)) * f(b, e + 0xd4)) * f(b, e + 0xec))
        / f(b, 0x1f5c);
    if byte(b, 0x1558) != 0 {
        base = f(b, 0x1178) * base;
    }
    base = base / ((f(b, 0x1fa0) / f(b, 0x1f98)) * f(b, 0x1f9c) + 1.);
    if b[0xdec / 4] == 2 {
        base = base * 2.;
    }
    let friction = ((f(b, e + 0xb4) * 3000.) * f(b, e + 0xe4)) * f(b, e + 0xe0);
    let mass = f(b, e) * 5.;
    let resistance = |speed: f32| {
        if speed > 0. {
            -((((speed - idle).abs() * c(0x3d088889)).min(speed.abs() * c(0x3d088889)) * friction)
                * mass)
        } else {
            ((speed.abs() * c(0x3d088889)) * friction) * mass
        }
    };
    if speed < idle * c(0x3f666666) {
        blend = 0.;
    }
    let mut torque = (1. - blend) * resistance(speed) + base * blend;
    if f(b, 0x1fa0) == 0. {
        torque = resistance(speed + idle);
        set(b, e + 0xbc, 0.);
        set_byte(b, e + 0xc0, 0);
    }
    torque = torque / speed.abs().max(20.);
    if byte(b, e + 0xc0) != 0 && speed < idle * 1.25 {
        torque = torque.max(((f(b, e + 0xa8) * f(b, e)) * f(b, e + 0xc4)) * c(0x43a6aa3d));
        if speed < idle * 0.3125 {
            let random = (draw(b) & 255) as f32;
            set(
                b,
                e + 0xc4,
                (random * c(0x3f99999a)) * c(0x3b808081) + c(0x3ecccccd),
            );
        }
    }
    let period = c(0x4544597c);
    let negative = c(0xc544597c);
    let mut phase = speed * delta + f(b, e + 0xf8);
    if !phase.is_finite() || phase.abs() > 2_000_000. {
        return Err(Error("engine phase exceeds bound"));
    }
    while phase > period {
        phase = phase - period;
    }
    while phase < negative {
        phase = phase + period;
    }
    set(b, e + 0xf8, phase);
    let t = (delta * 3.).clamp(0., 1.);
    set(b, e + 0x104, (1. - t) * f(b, e + 0x104) + speed * t);
    if !torque.is_finite() {
        return Err(Error("engine torque exceeds finite range"));
    }
    Ok(torque)
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed value-first sums"
)]
fn differential(s: &mut Frame, delta: f32) -> Result<(), Error> {
    let (b, v): (&mut [u32], &mut [u32]) = (&mut s.body, &mut s.view);
    let accelerating = byte(b, 0x164c) != 0;
    let o = if accelerating { 0x1570 } else { 0x157c };
    let mut mix = f(b, o);
    let mut rear = f(b, o + 4);
    let mut front = f(b, o + 8);
    let mut normalized = byte(b, 0x1588) != 0;
    for &i in &s.driven {
        if s.material_multipliers[i].is_none() {
            mix = 1.;
            if f(b, wheel(i) + 8) <= 0. {
                rear = 60.;
            } else {
                front = 60.;
            }
            normalized = false;
        }
    }
    let rear_mean = (f(b, wheel(1) + 0x104) + f(b, wheel(0) + 0x104)) * 0.5;
    let front_mean = (f(b, wheel(3) + 0x104) + f(b, wheel(2) + 0x104)) * 0.5;
    let mean = (rear_mean + front_mean) * 0.5;
    if byte(b, 0x1640) != 0 {
        mix = 0.;
    }
    let keep = 1. - mix;
    let target_front = keep * front_mean + mean * mix;
    let mut total = 0.;
    for i in 0..4 {
        let w = wheel(i);
        let value = if !s.driven.contains(&i) {
            0.
        } else if !normalized {
            f(b, w + 0x10c)
        } else {
            f(b, w + 0x10c) * if mean <= f(b, w + 0x104) { 5. } else { 6. }
        };
        set(v, 0x7c + i * 4, value);
        total = value + total;
    }
    if normalized {
        if !total.is_finite() || total == 0. {
            return Err(Error("singular differential weights"));
        }
        for i in 0..4 {
            set(v, 0x7c + i * 4, (1. / total) * f(v, 0x7c + i * 4));
        }
    }
    let t = (front * delta).clamp(0., 1.);
    for i in 2..4 {
        set(
            b,
            wheel(i) + 0x104,
            (1. - t) * f(b, wheel(i) + 0x104) + t * target_front,
        );
    }
    let t = (rear * delta).clamp(0., 1.);
    let target_rear = keep * rear_mean + mean * mix;
    let mut weights = 0.;
    let mut adjusted = 0.;
    for i in 0..2 {
        let w = wheel(i);
        let old = f(b, w + 0x104);
        let new = (1. - t) * old + t * target_rear;
        if f(b, 0x1590) > 0. && s.driven.contains(&i) && old > 0. {
            let change = (new - old).abs() - f(b, 0x158c);
            let extra = (change * f(b, 0x1590)).clamp(0., f(b, 0x1594) - 1.);
            let value = if new <= old {
                f(b, w + 0x10c) / (extra + 1.)
            } else {
                f(b, w + 0x10c) * (extra + 1.)
            };
            set(v, 0x7c + i * 4, value.clamp(0., 1.));
        }
        weights = f(b, w + 0x10c) + weights;
        adjusted = f(v, 0x7c + i * 4) + adjusted;
        set(b, w + 0x104, new);
    }
    if adjusted > 0. {
        set(v, 0x7c, f(v, 0x7c) * (weights / adjusted));
        set(v, 0x80, (weights / adjusted) * f(v, 0x80));
    }
    for i in 0..4 {
        if (b[0x1850 / 4].wrapping_sub(3) > 1) != (f(b, wheel(i) + 8) > 0.) {
            set(v, 0x7c + i * 4, f(b, 0x2264) * f(v, 0x7c + i * 4));
        }
    }
    Ok(())
}

fn traction(s: &mut Frame) {
    let (b, v) = (&mut s.body, &mut s.view);
    set_byte(b, 0x198c, 0);
    set(v, 0x9c, f32::MAX);
    for i in 0..4 {
        set(v, 0x8c + i * 4, 0.);
    }
    let mode = b[0x1988 / 4];
    let torque = f(v, 0x58);
    if (mode == 1 && f(b, 0x185c) == 1.)
        || (mode != 1 && mode != 2)
        || torque <= 0.
        || b[0x196c / 4] == 0
    {
        return;
    }
    let mut requested = [0.; 4];
    let mut available = [0.; 4];
    let mut factor = 0_f32;
    for i in 0..4 {
        let w = wheel(i);
        requested[i] = f(v, 0x7c + i * 4) * torque;
        if requested[i] > 0. {
            available[i] = if f(b, w + 0xa4) <= 0. {
                (f(v, 0x5c + i * 4) * c(0x3f7d70a4)) * f(b, w + 0xe0)
            } else {
                0.
            };
            factor = factor.max(available[i] / requested[i]);
        }
    }
    if factor > 1. {
        factor = 1.;
    } else {
        set_byte(b, 0x198c, 1);
    }
    set(v, 0x9c, torque * factor);
    for i in 0..4 {
        let force = requested[i] * factor;
        if force > 0. && available[i] < force {
            set_byte(b, 0x198c, 1);
            set(
                v,
                0x8c + i * 4,
                (force - available[i]).min(f(b, wheel(i) + 0x108)),
            );
        }
    }
}

/// Execute on a clone. Errors preserve body, view, solver and diagnostics.
#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed factor-first arithmetic"
)]
pub fn apply(frame: &mut Frame, roll: [Option<f32>; 4], delta: f32) -> Result<(), Error> {
    let b = &frame.body;
    let engine_offsets = [
        0, 4, 8, 0xa8, 0xb4, 0xbc, 0xc4, 0xc8, 0xd0, 0xd4, 0xdc, 0xe0, 0xe4, 0xe8, 0xec, 0xf4,
        0xf8, 0xfc, 0x100, 0x104, 0x108, 0x10c,
    ];
    let scalars = [
        0x1178, 0x1f5c, 0x1f98, 0x1f9c, 0x1fa0, 0x22c4, 0x22c8, 0x1570, 0x1574, 0x1578, 0x157c,
        0x1580, 0x1584, 0x158c, 0x1590, 0x1594, 0x2264, 0x1f70,
    ];
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || b[0x196c / 4] >= 16
        || b[0x1988 / 4] > 2
        || frame.driven.is_empty()
        || frame.driven.len() > 4
        || frame
            .driven
            .iter()
            .enumerate()
            .any(|(i, &w)| w >= 4 || frame.driven[..i].contains(&w))
        || engine_offsets
            .into_iter()
            .any(|o| !f(b, 0x17a0 + o).is_finite())
        || (0..32).any(|i| !f(b, 0x17ac + i * 4).is_finite())
        || scalars.into_iter().any(|o| !f(b, o).is_finite())
        || f(b, 0x1f5c) <= 0.
        || f(b, 0x1f98) <= 0.
        || 1. + (f(b, 0x1fa0) / f(b, 0x1f98)) * f(b, 0x1f9c) == 0.
        || f(b, 0x1894).abs() > 1_000_000.
        || f(b, 0x1898).abs() > 1_000_000.
        || (f(b, 0x1590) > 0. && f(b, 0x1594) < 1.)
        || roll.iter().enumerate().any(|(i, m)| {
            m.is_some() != frame.material_multipliers[i].is_some()
                || m.is_some_and(|x| !x.is_finite())
        })
        || (0..4).any(|i| {
            [
                0x78, 0x94, 0x98, 0x9c, 0xa0, 0xa4, 0xac, 0x100, 0x104, 0x108, 0x10c, 0x110,
            ]
            .into_iter()
            .any(|o| !f(b, wheel(i) + o).is_finite())
                || f(b, wheel(i) + 0x104).abs() > 1_000_000.
                || f(b, wheel(i) + 0x110).abs() > 1_000_000.
        })
    {
        return Err(Error("invalid post-control propulsion input"));
    }
    let mut s = frame.clone();
    let gear = s.body[0x196c / 4] as usize;
    let mut ratio = f(&s.body, 0x18f4 + gear * 4);
    if ratio == 0. {
        ratio = 1.;
    }
    if !ratio.is_finite() {
        return Err(Error("invalid propulsion ratio"));
    }
    let torque = (engine(&mut s.body, delta)? as f64 / ratio as f64) as f32;
    set(&mut s.view, 0x58, torque);
    set(&mut s.body, 0x1f60, torque);
    differential(&mut s, delta)?;
    traction(&mut s);
    let t = (delta * 10.).min(1.);
    let keep = 1. - t;
    for i in 0..4 {
        let w = wheel(i);
        let b: &mut [u32] = &mut s.body;
        set(b, w + 0xac, keep * f(b, w + 0xac) + f(b, w + 0xa4) * t);
        set(b, w + 0x94, keep * f(b, w + 0x94) + t * f(b, w + 0x9c));
        set(b, w + 0x98, f(b, w + 0xa0) * t + f(b, w + 0x98) * keep);
        for o in [0xa4, 0x9c, 0xa0] {
            set(b, w + o, 0.);
        }
        let mut angle = f(b, w + 0x104) * delta + f(b, w + 0x110);
        if angle > c(0x439d1463) {
            angle = angle - c(0x441d1463);
        }
        if angle < c(0xc39d1463) {
            angle = angle + c(0x441d1463);
        }
        set(b, w + 0x110, angle);
    }
    let points: [u32; 16] = s.view[0x250 / 4..0x290 / 4].try_into().unwrap();
    for (i, roll) in roll.into_iter().enumerate() {
        if let Some(roll) = roll {
            let amount = f(&s.body, 0x1f70) * roll;
            for j in 0..4 {
                let o = 0x250 + i * 16 + j * 4;
                let value = f(&s.view, o) - amount * f(&s.body, 0x15f0 + j * 4);
                set(&mut s.view, o, value);
            }
            s.view[(0x250 + i * 16 + 12) / 4] = 1_f32.to_bits();
        }
    }
    vehicle_force::apply(&mut s, delta)?;
    s.view[0x250 / 4..0x290 / 4].copy_from_slice(&points);
    s.body[0x15a8 / 4..0x15b4 / 4].copy_from_slice(&s.view[0x48 / 4..0x54 / 4]);
    *frame = s;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_force_failure_rolls_back_engine_differential_and_solver() {
        let mut frame = Frame {
            body: [0; vehicle_force::BODY_WORDS],
            view: [0; vehicle_force::VIEW_WORDS],
            solver: [0xdeadbeef; 884],
            driven: vec![2, 3],
            material_multipliers: [None; 4],
            diagnostics: [1.; 8],
        };
        frame.body[0x1598 / 4] = 1;
        frame.body[0x1620 / 4] = 4;
        frame.body[0x196c / 4] = 1;
        for (o, x) in [
            (0x17a0, 0.22),
            (0x17a4, 900.),
            (0x17a8, 105.),
            (0x1f5c, 1.),
            (0x1f98, 3600.),
            (0x1fa0, 3600.),
        ] {
            set(&mut frame.body, o, x);
        }
        let before = frame.clone();
        let error = apply(&mut frame, [None; 4], 0.01).unwrap_err();
        assert_eq!(error.0, "nonfinite or invalid physical force inputs");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
}
