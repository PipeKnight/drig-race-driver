//! Actual post-force driver interval through speed, RPM, camber and assists.
//! The following chassis-motion routine and earlier world scheduling are separate.
use crate::{Error, spawn, vehicle_force::BODY_WORDS};

const FIELDS: &[usize] = &[
    0xe1c, 0xe20, 0xe74, 0x15a4, 0x17a4, 0x17a8, 0x185c, 0x1894, 0x18fc, 0x1974, 0x1f00, 0x1fa0,
    0x226c, 0x2274,
];
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, x: f32) {
    b[o / 4] = x.to_bits();
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> ((o % 4) * 8)) as u8
}
fn v(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[2] * b[2] + a[3] * b[3]) + (a[0] * b[0] + a[1] * b[1])
}

/// Clone/commit preserves all incoming words on an input or arithmetic failure.
/// Constants are those of the identified reference image, including its selected
/// orientation and wheel-response globals; no arbitrary captured pointer is followed.
pub fn apply(body: &mut [u32; BODY_WORDS], delta: f32) -> Result<(), Error> {
    if !delta.is_finite() || delta <= 0. || delta > 0.1 || body[0x1620 / 4] != 4 {
        return Err(Error("invalid post-force delta"));
    }
    if FIELDS.iter().any(|&o| !f(body, o).is_finite())
        || (0x15c0..0x1610).step_by(4).any(|o| !f(body, o).is_finite())
        || (0..4).any(|i| {
            [0, 0x40, 0x138, 0x13c]
                .into_iter()
                .any(|o| !f(body, 0x1990 + i * 0x150 + o).is_finite())
        })
    {
        return Err(Error("invalid post-force scalar"));
    }
    for (bit, i) in [(1, 0), (2, 2)] {
        if body[0x21f4 / 4] & bit != 0 {
            let w = 0x1990 + i * 0x150;
            let ratio =
                (f(body, w + 0x40) - f(body, w + 0x190)) / (f(body, w) - f(body, w + 0x150));
            if !ratio.is_finite() {
                return Err(Error("invalid camber span"));
            }
        }
    }
    let mut next = *body;
    let b = &mut next;
    let velocity = v(b, 0x15c0);
    let square = dot(velocity, velocity);
    if !square.is_finite() {
        return Err(Error("invalid post-force speed"));
    }
    let speed = if square == 0. {
        0.
    } else {
        let seed = spawn::reference_seed(square, false)?;
        let product = seed * square;
        ((1. - product * seed) * (product * 0.5)) + product
    };
    set(b, 0x1648, speed);
    if f(b, 0x1974) < 0.25 && byte(b, 0x1970) == 0 && byte(b, 0x1971) != 0 && f(b, 0x1fa0) > 0. {
        let minimum = f(b, 0x17a8) * f32::from_bits(0x3f8147ae);
        if f(b, 0x1894) < minimum {
            set(b, 0x1894, minimum);
        }
    }
    if byte(b, 0x1971) != 0 {
        idle(b, delta);
    }
    let rpm = f(b, 0x1894);
    set(b, 0x1f00, rpm);
    if f(b, 0x226c) != 0. {
        for i in 0..4 {
            for o in [0x94, 0x98, 0x9c, 0xa0, 0xa4, 0xac] {
                set(b, 0x1990 + i * 0x150 + o, 0.);
            }
        }
        if f(b, 0x1894) < f(b, 0x17a8) {
            set(b, 0x1894, 0.);
        }
    }
    for (bit, i) in [(1, 0), (2, 2)] {
        if b[0x21f4 / 4] & bit != 0 {
            let w = 0x1990 + i * 0x150;
            let ratio = (f(b, w + 0x40) - f(b, w + 0x190)) / (f(b, w) - f(b, w + 0x150));
            let angle = f64::from(ratio).atan() as f32;
            set(b, w + 0xec, -0.0 - angle);
            set(b, w + 0x23c, angle);
        }
    }
    if byte(b, 0x1f2a) != 0 {
        let side = v(b, 0x15e0);
        let forward = v(b, 0x1600);
        for (axis, height, positive) in [(forward, side[1], false), (side, forward[1], true)] {
            if height.abs() > 0.5 {
                let mut factor = ((height.abs() - 0.5) * f32::from_bits(0x40dfc6b6)).clamp(0., 1.);
                let angular = v(b, 0x15d0);
                let projection = dot(axis, angular) * height;
                if (positive && projection > 0.) || (!positive && projection < 0.) {
                    let mut angular = angular;
                    if factor >= 1. {
                        angular[..3].fill(0.);
                    }
                    if (positive && height >= 0.) || (!positive && height <= 0.) {
                        factor = -0.0 - factor;
                    }
                    let amount = (factor * delta) * 10.;
                    for j in 0..3 {
                        set(b, 0x15d0 + j * 4, amount * axis[j] + angular[j]);
                    }
                    set(b, 0x15dc, 0.);
                }
            }
        }
    }
    let brake = f(b, 0xe20);
    let timer = if byte(b, 0x1559) == 0
        || brake <= f32::from_bits(0x3f666666)
        || f(b, 0xe1c) >= f32::from_bits(0x3dcccccd)
    {
        0.
    } else {
        f(b, 0xe74) + delta
    };
    set(b, 0xe74, timer);
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        let level = ((f(b, 0x15a4) * f32::from_bits(0x3f2df8dd)) * f(b, w + 0x138)).clamp(0., 1.);
        let speed = ((f(b, 0x1648) - 2.) * 0.5).clamp(0., 1.);
        let activity = (speed * level) * brake;
        let old = f(b, w + 0x13c);
        let value = if activity <= 0. {
            let difference = (old - f32::from_bits(0x43929333)) * f32::from_bits(0x3ad00d01);
            ((1. - delta * f32::from_bits(0x3eaa7efa)) * difference) * 630.
                + f32::from_bits(0x43929333)
        } else {
            let difference = (f32::from_bits(0x4466c99a) - old) * f32::from_bits(0x3ad00d01);
            f32::from_bits(0x4466c99a)
                - ((1. - (activity * delta) * f32::from_bits(0x3f333333)) * difference) * 630.
        };
        set(b, w + 0x13c, value);
    }
    if f(b, 0x2274) != 0. {
        set(b, 0x1894, 0.);
    }
    if [
        0x1648, 0x1894, 0x1f00, 0xe74, 0x15d0, 0x15d4, 0x15d8, 0x15dc,
    ]
    .into_iter()
    .any(|o| !f(b, o).is_finite())
        || (0..4).any(|i| !f(b, 0x1990 + i * 0x150 + 0x13c).is_finite())
    {
        return Err(Error("nonfinite post-force result"));
    }
    *body = next;
    Ok(())
}

fn idle(b: &mut [u32], delta: f32) {
    let rpm = f(b, 0x1894);
    let idle = f(b, 0x17a8);
    let low = idle * f32::from_bits(0x3f733333);
    let target = if f(b, 0x18fc) == 0. || low <= rpm {
        let mut target = idle * f32::from_bits(0x3f8147ae);
        if b[0x196c / 4] == 1 {
            let ceiling = if f(b, 0x18fc) == 0. {
                idle * 1.5
            } else {
                f(b, 0x17a4) * 0.5
            };
            if rpm < ceiling {
                let throttle = (f(b, 0x185c) - 0.5) * 2.;
                if throttle > 0. {
                    let candidate =
                        (throttle.clamp(-10., 1.) * ((ceiling - rpm) * 2.)) * delta + f(b, 0x1f00);
                    if target < candidate {
                        target = candidate;
                    }
                }
            }
        }
        target
    } else if byte(b, 0x1860) != 0 {
        low * delta + f(b, 0x1f00)
    } else {
        0.
    };
    if f(b, 0x1fa0) > 0. && f(b, 0x226c) == 0. && rpm < target {
        set(b, 0x1894, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> [u32; BODY_WORDS] {
        let mut b = [0; BODY_WORDS];
        b[0x1620 / 4] = 4;
        for (o, x) in [(0x17a8, 105.), (0x17a4, 900.), (0x15a4, 0.001)] {
            set(&mut b, o, x);
        }
        for i in 0..4 {
            set(
                &mut b,
                0x1990 + i * 0x150,
                if i % 2 == 0 { -1. } else { 1. },
            );
        }
        b
    }
    #[test]
    fn disabled_rpm_history_precedes_clear_and_speed_consumes_four_lanes() {
        let mut b = body();
        set(&mut b, 0x1894, 100.);
        set(&mut b, 0x226c, 1.);
        set(&mut b, 0x15cc, 5.);
        for i in 0..4 {
            for o in [0x94, 0x98, 0x9c, 0xa0, 0xa4, 0xac] {
                set(&mut b, 0x1990 + i * 0x150 + o, 3.);
            }
        }
        apply(&mut b, 0.01).unwrap();
        assert_eq!(f(&b, 0x1f00), 100.);
        assert_eq!(f(&b, 0x1894), 0.);
        assert!((f(&b, 0x1648) - 5.).abs() < 0.000001);
        for i in 0..4 {
            for o in [0x94, 0x98, 0x9c, 0xa0, 0xa4, 0xac] {
                assert_eq!(b[(0x1990 + i * 0x150 + o) / 4], 0);
            }
        }
    }
    #[test]
    fn late_response_overflow_rolls_back_speed_camber_and_rpm_history() {
        let mut b = body();
        b[0x21f4 / 4] = 3;
        set(&mut b, 0x1894, 100.);
        set(&mut b, 0x15c0, 10.);
        set(&mut b, 0x15a4, 1.);
        set(&mut b, 0xe20, f32::MAX);
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            set(&mut b, w + 0x40, i as f32 * 0.1);
            set(&mut b, w + 0x138, 10.);
            set(&mut b, w + 0x13c, -f32::MAX);
        }
        let before = b;
        assert_eq!(
            apply(&mut b, 0.1).unwrap_err().0,
            "nonfinite post-force result"
        );
        assert_eq!(b, before);
    }
}
