//! Selected assisted-automatic driver preparation, including clutch engagement.
//! Respawn/nitro and neutral/manual operation require separate contracts.
use crate::{Error, dynamics, spawn, vehicle_force::Frame};
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
    let shift = (o % 4) * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | ((x as u32) << shift);
}
fn v(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
fn store(b: &mut [u32], o: usize, x: [f32; 4]) {
    for (i, x) in x.into_iter().enumerate() {
        set(b, o + i * 4, x);
    }
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[2] * b[2] + a[3] * b[3]) + (a[0] * b[0] + a[1] * b[1])
}
fn w(i: usize) -> usize {
    0x1990 + i * 0x150
}
fn c(x: u32) -> f32 {
    f32::from_bits(x)
}
fn speed(a: [f32; 4]) -> Result<f32, Error> {
    let square = dot(a, a);
    if !square.is_finite() {
        return Err(Error("invalid preparation speed"));
    }
    if square == 0. {
        return Ok(0.);
    }
    let seed = spawn::reference_seed(square, false)?;
    let product = seed * square;
    Ok((1. - product * seed) * (product * 0.5) + product)
}
/// Caller supplies gravity and the previous wheel materials' downforce flags.
/// All frame storage is committed together; no source/world or pose advance here.
#[allow(clippy::assign_op_pattern, clippy::neg_multiply)] // Observed float operand order.
pub fn apply(
    frame: &mut Frame,
    delta: f32,
    gravity: f32,
    high_downforce: [bool; 4],
) -> Result<(), Error> {
    let b = &frame.body;
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || !gravity.is_finite()
        || gravity < 0.
        || byte(b, 0x24a) != 0
        || b[0x2278 / 4] != 0
        || byte(b, 0xde4) != 0
        || byte(b, 0xdfc) != 0
        || !matches!(b[0x1598 / 4], 1 | 2)
        || !(1..=9).contains(&b[0x196c / 4])
        || byte(b, 0x1970) == 0
        || byte(b, 0x1971) == 0
        || b[0x3c0 / 4] >= 5
        || b[0x2e0 / 4] >= 10
        || frame.driven.is_empty()
        || frame.driven.len() > 4
        || frame
            .driven
            .iter()
            .enumerate()
            .any(|(i, &x)| x >= 4 || frame.driven[..i].contains(&x))
    {
        return Err(Error("unsupported driver preparation state"));
    }
    let fields = [
        0x1568, 0xe1c, 0xe20, 0xe34, 0x159c, 0x2e4, 0x1648, 0x1f14, 0x1f34, 0x15a0, 0x15a4, 0x15a8,
        0x15ac, 0x15b0, 0x17a0, 0x17a4, 0x185c, 0x187c, 0x1890, 0x1894, 0x1f94, 0x1fa0, 0x110c,
        0x226c, 0x1ee4, 0x1ee8, 0x1eec, 0x1ef0, 0xe08, 0xe0c, 0xe10, 0xe14, 0x1f04, 0x1f08, 0x1f18,
        0x1f1c, 0x1f64, 0x1f68, 0x139c, 0x21c4, 0x2214, 0x2268,
    ];
    if fields.into_iter().any(|o| !f(b, o).is_finite())
        || (0x15c0..0x1620)
            .step_by(4)
            .chain((0x2ec..0x304).step_by(4))
            .chain((0x1f4c..0x1f5c).step_by(4))
            .any(|o| !f(b, o).is_finite())
        || (0..4).any(|i| {
            [8, 0x78, 0x100, 0x104, 0x10c, 0xe0, 0xa0, 0x9c, 0x12c]
                .into_iter()
                .any(|o| !f(b, w(i) + o).is_finite())
        })
        || [0x15a0, 0x15a4, 0x15a8, 0x15ac, 0x15b0, 0x17a0, 0x17a4]
            .into_iter()
            .any(|o| f(b, o) <= 0.)
        || (0..4).any(|i| f(b, w(i) + 0xe0) <= 0. || f(b, w(i) + 0x100) <= 0.)
        || f(b, 0x1ef0) == f(b, 0x1eec)
        || f(b, 0xe14) >= 1.
        || !(0. ..=2.).contains(&f(b, 0x2ec))
        || !(0. ..=2.).contains(&f(b, 0x2f0))
    {
        return Err(Error("invalid driver preparation operand"));
    }
    let mut next = frame.clone();
    let b: &mut [u32] = &mut next.body;
    set(b, 0x1568, delta + f(b, 0x1568));
    if byte(b, 0x1108) != 0 {
        set(b, 0x1884, 1.);
        let gear = b[0x196c / 4];
        if gear != 9 {
            let mut counts = [0u32; 2];
            let mut sums = [0f32; 2];
            for i in 0..4 {
                let group = usize::from(f(b, w(i) + 0x10c) > 0.);
                let value = f(b, w(i) + 0x104) * f(b, w(i) + 0xe0);
                sums[group] = value + sums[group];
                counts[group] += 1;
            }
            if counts[0] > 0
                && counts[1] > 0
                && sums[1] / (counts[1] as f32) < sums[0] / (counts[0] as f32) - c(0x3e4ccccd)
            {
                set(b, 0x1884, f(b, 0xe20));
            }
        }
    }
    let ring = b[0x3c0 / 4] as usize;
    set(b, 0x3ac + ring * 4, delta);
    b[0x3c0 / 4] = ((ring + 1) % 5) as u32;
    b.copy_within(0x15c0 / 4..0x15e0 / 4, 0xb20 / 4);
    let damp = !(b[0x1f74 / 4] == 1 && b[0x196c / 4] == 1);
    let mut airborne = false;
    if f(b, 0x1f34) != 0. {
        for (i, &wheel) in next.driven.iter().enumerate() {
            let wheel = w(wheel);
            if b[(wheel + 0xc0) / 4] == 0 {
                if damp {
                    let spin = if byte(b, 0x1f48 + i) != 0 {
                        (1. - delta * f(b, 0x1f34)) * f(b, 0x1f4c + i * 4)
                    } else {
                        (f64::from(dot(v(b, 0x15c0), v(b, 0x1600))) / f64::from(f(b, wheel + 0xe0)))
                            as f32
                    };
                    set(b, wheel + 0x104, spin);
                }
                airborne = true;
                put_byte(b, 0x1f48 + i, 1);
                set(b, 0x1f4c + i * 4, f(b, wheel + 0x104));
            } else {
                put_byte(b, 0x1f48 + i, 0);
            }
        }
        if airborne && damp {
            set(b, 0xe1c, 0.);
        }
    }
    if f(b, 0x15f4) < 0. {
        for i in 0..4 {
            if b[(w(i) + 0xc0) / 4] == 0 {
                set(b, w(i) + 0x104, (1. - delta) * f(b, w(i) + 0x104));
            }
        }
    }
    for i in 0..4 {
        let inverse = 1. / (f(b, 0x15a0) * c(0x401cf5c3));
        let lateral = f(b, w(i) + 0x9c) * inverse;
        let mut longitudinal = f(b, w(i) + 0xa0) * inverse;
        if dot(v(b, 0x15c0), v(b, 0x1600)) < 0. {
            longitudinal *= -1.;
        }
        let magnitude = (longitudinal * longitudinal + lateral * lateral).sqrt();
        let amount = ((magnitude - f(b, 0x1eec)) / (f(b, 0x1ef0) - f(b, 0x1eec))).clamp(0., 1.);
        let reverse = if longitudinal < 0. {
            ((longitudinal.abs() / magnitude - c(0x3f4ccccd)) * 5.).clamp(0., 1.)
        } else {
            0.
        };
        let factor = (1. - f(b, 0x1ee4) * amount).min(1. - (f(b, 0x1ee8) * reverse) * amount);
        set(&mut next.view, 0x298 + i * 4, factor);
        set(b, w(i) + 0x74, factor);
    }
    clutch(b, &next.driven)?;
    set(b, 0x159c, f(b, 0x159c) + delta);
    set(b, 0x2e4, f(b, 0x2e4) + delta);
    b.copy_within(0x15c0 / 4..0x15d0 / 4, 0x1fc0 / 4);
    let current = speed(v(b, 0x15c0))?;
    let rising = current > f(b, 0x1648) || f(b, 0xe20) <= f(b, 0xe1c);
    put_byte(b, 0x164c, u8::from(rising));
    set(b, 0x1648, current);
    let ring = b[0x2e0 / 4] as usize;
    set(b, 0x290 + ring * 4, current);
    set(b, 0x2b8 + ring * 4, current);
    b[0x2e0 / 4] = ((ring + 1) % 10) as u32;
    if byte(b, 0x1f78) != 0 {
        let amount = ((12. - current) * 0.25).clamp(0., 1.);
        if amount > 0. {
            let angular = ((f(b, 0x15d4).abs() - 3.) * c(0x3eaaaaab)).clamp(0., 1.);
            let factor = 1. - ((angular * amount) * delta) * 2.;
            store(b, 0x15d0, v(b, 0x15d0).map(|x| factor * x));
        }
    }
    set(&mut next.view, 0x2a8, 0.);
    set(b, 0x1f14, f(b, 0x1f14) + delta);
    let burn = if f(b, 0x187c) < c(0x3e99999a) {
        1.5
    } else {
        f(b, 0x185c) * c(0x3f4ccccd) + c(0x3e4ccccd)
    };
    if byte(b, 0xb10) != 0 {
        let amount = ((((f(b, 0x1894) / f(b, 0x17a4)) + 1.) * 0.5) * burn) * f(b, 0x1f94) * delta;
        set(b, 0x1fa0, f(b, 0x1fa0) - amount);
    }
    if f(b, 0x1fa0) < 0. {
        set(b, 0x1fa0, 0.);
    }
    // 9969C0 and 9B9DA0 are no-ops under the explicit inactive nitro/respawn guards.
    if byte(b, 0x21f2) != 0 {
        if current < 1.5 && f(b, 0x226c) == 0. {
            let amount = (f(b, 0xe20) * delta) * -1.;
            store(
                b,
                0x15c0,
                std::array::from_fn(|i| amount * f(b, 0x1600 + i * 4) + f(b, 0x15c0 + i * 4)),
            );
        }
        if current < c(0x3fcccccd) && dot(v(b, 0x15c0), v(b, 0x1600)) <= c(0x3dcccccd) {
            set(b, 0xe20, 0.);
        }
    }
    aero(b, delta, gravity, high_downforce)?;
    oscillator(b, delta);
    let previous = f(b, 0x1890);
    set(b, 0x1890, f(b, 0x1894));
    let axis = if [2, 4].contains(&b[0x1850 / 4]) {
        2
    } else {
        0
    };
    let amount = (f(b, 0x1648) * c(0x3d23d70a)).clamp(0., 1.);
    let reaction = ((f(b, 0x1894) - previous) * f(b, 0x17a0)) / f(b, 0x15a8 + axis * 4);
    let gain = (1. - amount) * f(b, 0x110c) + 1.;
    store(
        b,
        0x15d0,
        std::array::from_fn(|i| {
            (gain * f(b, 0x15e0 + axis * 16 + i * 4)) * reaction + f(b, 0x15d0 + i * 4)
        }),
    );
    if fields
        .into_iter()
        .chain((0x15c0..0x15e0).step_by(4))
        .chain((0x2ec..0x304).step_by(4))
        .chain((0x1f4c..0x1f5c).step_by(4))
        .chain((0..4).map(|i| w(i) + 0x104))
        .chain([0x1884, 0x1f10, 0x1fa0])
        .filter(|o| o % 4 == 0)
        .any(|o| !f(b, o).is_finite())
        || next.view[0x298 / 4..0x2ac / 4]
            .iter()
            .any(|&x| !f32::from_bits(x).is_finite())
    {
        return Err(Error("nonfinite driver preparation output"));
    }
    *frame = next;
    Ok(())
}
// Selected 98E1B0 call: assisted automatic, a non-neutral gear, modes one/two.
#[allow(clippy::assign_op_pattern)] // Keep wheel-first sums from the SIMD helper.
fn clutch(b: &mut [u32], driven: &[usize]) -> Result<(), Error> {
    if byte(b, 0x1640) != 0 {
        b[0x1598 / 4] = 2;
        return Ok(());
    }
    if b[0x1598 / 4] == 1 {
        return Ok(());
    }
    let ratio = f(b, 0x18f4 + 4 * b[0x196c / 4] as usize);
    if !ratio.is_finite() || !f(b, 0x17a8).is_finite() {
        return Err(Error("invalid clutch engagement input"));
    }
    if ratio != 0. {
        let inertia = f(b, 0x17a0) / (ratio * ratio);
        let mut spin = 0.;
        let mut wheels = 0.;
        for &i in driven {
            spin = f(b, w(i) + 0x104) + spin;
            wheels = f(b, w(i) + 0x100) + wheels;
        }
        let difference = f(b, 0x1894) * ratio - spin / driven.len() as f32;
        let blend = 1. - inertia / (wheels + inertia);
        if !difference.is_finite() || !blend.is_finite() {
            return Err(Error("invalid clutch engagement arithmetic"));
        }
        if ratio * difference < 0. || f(b, 0x17a8) * c(0x3f8ccccd) < f(b, 0x1894) {
            let speed = f(b, 0x1894) - (blend / ratio) * difference;
            set(b, 0x1894, speed);
            let change = (1. - blend) * difference;
            for &i in driven {
                set(b, w(i) + 0x104, change + f(b, w(i) + 0x104));
            }
        }
    }
    b[0x1598 / 4] = 1;
    Ok(())
}
fn oscillator(b: &mut [u32], delta: f32) {
    let phase = f(b, 0x2f0);
    let mut x = phase + f(b, 0x2ec);
    if x > 2. {
        x -= 2.;
    }
    if x > 1. {
        x = 2. - x;
    }
    if x > 0.5 {
        x = 1. - x;
    }
    if f(b, 0x2fc) > 0. {
        x = x.min(phase).min(2. - phase);
    }
    let blend = ((x * 4.).sqrt() * c(0x3f666666) + c(0x3dcccccd)).clamp(0., 1.);
    let rate = f(b, 0x2f4);
    let phase = (rate * blend) * delta + phase;
    let timer = f(b, 0x2f8) + delta;
    set(b, 0x2f8, timer);
    set(b, 0x2f0, phase);
    if rate == 0. {
        if timer > f(b, 0x2fc) && f(b, 0x300) != 0. {
            set(b, 0x2f4, 1. / f(b, 0x300));
            set(b, 0x2f8, 0.);
        }
    } else if phase >= 2. {
        set(b, 0x2f8, 0.);
        if f(b, 0x2fc) <= 0. && f(b, 0x300) != 0. {
            set(b, 0x2f0, phase - 2.);
            set(b, 0x2f4, 1. / f(b, 0x300));
        } else {
            set(b, 0x2f0, 0.);
            set(b, 0x2f4, 0.);
        }
    }
}
fn impulse(b: &mut [u32], amount: f32, point: [f32; 4]) -> Result<(), Error> {
    let up = v(b, 0x15f0);
    let (linear, angular) = dynamics::apply_impulse(
        dynamics::ImpulseBody {
            mass: f(b, 0x15a0),
            inertia: std::array::from_fn(|i| f(b, 0x15a8 + i * 4)),
            basis: std::array::from_fn(|i| v(b, 0x15e0 + i * 16)),
        },
        v(b, 0x15c0),
        v(b, 0x15d0),
        up.map(|x| amount * x),
        point,
    )?;
    store(b, 0x15c0, linear);
    store(b, 0x15d0, angular);
    Ok(())
}
#[allow(clippy::assign_op_pattern)] // Preserve original addition order.
fn aero(b: &mut [u32], delta: f32, gravity: f32, high: [bool; 4]) -> Result<(), Error> {
    set(
        b,
        0x15c4,
        (f64::from(f(b, 0x15c4)) - f64::from(gravity) * f64::from(delta)) as f32,
    );
    if f(b, 0x1f18) > 0. && f(b, 0x159c) - f(b, 0x1f1c) > c(0x3dcccccd) {
        set(b, 0x1f18, (f(b, 0x1f18) - delta * 10.).clamp(0., 1.));
    }
    let density = f(b, 0x21c4);
    for (axis, coefficient) in [(0x15e0, 0xe08), (0x15f0, 0xe0c)] {
        let projection = dot(v(b, 0x15c0), v(b, axis));
        let force = ((((projection.abs() * f(b, coefficient)) * projection) * density) * delta)
            * f(b, 0x15a4);
        let amount = -0.0 - force;
        store(
            b,
            0x15c0,
            std::array::from_fn(|i| amount * f(b, axis + i * 4) + f(b, 0x15c0 + i * 4)),
        );
    }
    let forward = v(b, 0x1600);
    let projection = dot(v(b, 0x15c0), forward);
    let mut drag = ((projection.abs() * f(b, 0xe10)) * projection) * density;
    if byte(b, 0x18c8) == 0 {
        let throttle = if b[0x196c / 4] == 0 || f(b, 0xe34) == 1. {
            1.
        } else {
            1. - f(b, 0x185c)
        };
        let mut rolling = 0.;
        for i in 0..4 {
            if b[(w(i) + 0xc0) / 4] != 0 {
                let mut amount = f(b, w(i) + 0x12c) / 4.;
                if byte(b, w(i) + 0x130) != 0 {
                    amount *= throttle;
                }
                rolling = amount + rolling;
            }
        }
        let rolling = (f(b, 0x15a0) * rolling) * 3.;
        drag = if projection <= 0. {
            drag - rolling
        } else {
            rolling + drag
        };
    }
    let amount = -0.0 - (drag * delta) * f(b, 0x15a4);
    store(
        b,
        0x15c0,
        std::array::from_fn(|i| f(b, 0x15c0 + i * 4) + amount * forward[i]),
    );
    let magnitude = speed(v(b, 0x15c0))?;
    let alignment = dot(v(b, 0x15c0).map(|x| (1. / magnitude) * x), forward);
    if alignment <= f(b, 0xe14) || alignment.is_nan() {
        set(b, 0x1f10, 0.);
        return Ok(());
    }
    let speed = ((alignment - f(b, 0xe14)) / (1. - f(b, 0xe14))) * magnitude;
    let airborne = (0..4).any(|i| f(b, w(i) + 0x78) == 0.);
    let mut main = f(b, 0x1f04);
    let mut extra = f(b, 0x1f08);
    let mut center = f(b, 0x1f64);
    if (0..4).any(|i| b[(w(i) + 0xc0) / 4] != 0 && high[i]) && main < 15. {
        main = 15.;
    }
    if byte(b, 0x1f79) != 0 && airborne {
        main = 0.;
        extra = 0.;
    }
    if byte(b, 0x21f0) == 0 && airborne {
        let amount = -0.0 - ((((f(b, 0x2214) * extra) * speed) * speed) * density) * delta;
        impulse(b, amount, forward.map(|x| c(0xbfe66666) * x))?;
    } else {
        main = extra + main;
        if byte(b, 0x21f0) != 0 && airborne {
            center = if f(b, 0x15c4) > 0. {
                f(b, 0x139c)
            } else {
                f(b, 0x1f68)
            };
        }
    }
    let force = ((((f(b, 0x2214) * main) * speed) * speed) * density) * f(b, 0x2268);
    let local_z = (1. - center) * f(b, w(0) + 8) + f(b, w(2) + 8) * center;
    let point = std::array::from_fn(|i| {
        (0. * f(b, 0x1610 + i * 4) + 0. * f(b, 0x15f0 + i * 4))
            + (local_z * f(b, 0x1600 + i * 4) + 0. * f(b, 0x15e0 + i * 4))
    });
    impulse(b, -0.0 - force * delta, point)?;
    set(b, 0x1f10, force);
    Ok(())
}
