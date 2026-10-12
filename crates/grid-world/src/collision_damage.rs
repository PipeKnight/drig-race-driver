//! Whole selected impact distribution with owned mass/pose and explicit global RNG.
//! Resource/constructor setup and notifications are separate from this dispatcher.
use crate::{Error, damage_region, vehicle_handling, vehicle_publication::State};

pub struct Output {
    pub state: State,
    pub random_state: u32,
    pub applied: bool,
}

pub fn calculate(
    input: &State,
    point: [f32; 4],
    impact: f32,
    wheel_mode: bool,
    random_state: u32,
) -> Result<Output, Error> {
    if point[3] != 1. {
        return Err(Error("invalid collision damage framing"));
    }
    calculate_with_queries(input, point, impact, wheel_mode, random_state, None)
}

/// Whole dispatcher with the surrounding handling producer's owned query list.
/// Its wheel-center forward transform may yield homogeneous W zero or one.
pub fn calculate_with_queries(
    input: &State,
    point: [f32; 4],
    impact: f32,
    wheel_mode: bool,
    random_state: u32,
    queries: Option<&[vehicle_handling::QueryRecord]>,
) -> Result<Output, Error> {
    let f = |b: &[u32], o: usize| f32::from_bits(b[o / 4]);
    if input.vehicle.len() != crate::vehicle_publication::VEHICLE_WORDS
        || point.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
        || !matches!(point[3], 0. | 1.)
        || queries.is_some_and(|rows| rows.len() > 64)
        || !(0. ..=1_000_000_000.).contains(&impact)
    {
        return Err(Error("invalid collision damage framing"));
    }
    let car = &input.vehicle;
    let m = vehicle_handling::MUTABLE;
    let b = &car[m / 4..(m + 0x460) / 4];
    let p = &input.physical;
    let mass = f(p, 0x3f0);
    if car[0xdb0 / 4] != 0
        || b[0x210 / 4] != 0
        || b[0x214 / 4] != 0
        || car[0x1620 / 4] != 4
        || (car[0x104d / 4] >> 8 & 255) > 1
        || !(1. ..=1_000_000.).contains(&mass)
        || p[0x1d0 / 4..0x210 / 4]
            .iter()
            .any(|&v| !f32::from_bits(v).is_finite() || f32::from_bits(v).abs() > 1000.)
        || (0..3).any(|j| p[(0x1d0 + j * 16 + 12) / 4] != 0)
        || (0..3).any(|j| (0..3).any(|k| f(p, 0x1d0 + j * 16 + k * 4).abs() > 1.1))
        || f(p, 0x20c) != 1.
        || b[0x218 / 4..0x380 / 4]
            .iter()
            .any(|&v| !f32::from_bits(v).is_finite())
        || b[0x430 / 4..0x43c / 4]
            .iter()
            .any(|&v| !(0. ..=1000.).contains(&f32::from_bits(v)))
        || !(0. ..=1.5).contains(&f(b, 0x42c))
        || (0x360..0x37c)
            .step_by(4)
            .any(|o| !(0. ..=1000.).contains(&f(b, o)))
        || (0..6).any(|i| !(0. ..=1.5).contains(&f(b, i * 4)))
        || (0..4).any(|i| !(0. ..=1.5).contains(&f(b, 0x28 + i * 4)))
    {
        return Err(Error("unsupported collision damage owner"));
    }
    let main_gain = (f(b, 0x438) * f(b, 0x430)) * f(b, 0x434);
    let secondary_gain = (f(b, 0x438) * f(b, 0x434)) * f(b, 0x430);
    if !(0. ..=1000.).contains(&main_gain) || !(0. ..=1000.).contains(&secondary_gain) {
        return Err(Error("unsupported collision damage gain"));
    }
    for (box_at, coefficients) in [
        (0x60, 0x240),
        (0x90, 0x240),
        (0xc0, 0x240),
        (0xf0, 0x240),
        (0x120, 0x340),
        (0x150, 0x218),
        (0x180, 0x22c),
        (0x1b0, 0x304),
        (0x1e0, 0x32c),
    ] {
        if b[(box_at + 32) / 4] & 255 > 1 {
            return Err(Error("invalid collision damage region enable"));
        }
        damage_region::calculate(damage_region::Input {
            previous: 0.,
            point: [point[0], point[1], point[2], 1.],
            impact: 0.,
            gain: 1.,
            enabled: false,
            random_state,
            coefficients: std::array::from_fn(|j| f(b, coefficients + j * 4)),
            center: std::array::from_fn(|j| f(b, box_at + j * 4)),
            half: std::array::from_fn(|j| f(b, box_at + 16 + j * 4)),
            radius: f(b, 0x370),
            factors: [f(b, 0x374), f(b, 0x378)],
        })?;
    }
    let at = if wheel_mode { 0x368 } else { 0x360 };
    if !(0. ..=1000.).contains(&f(b, at)) || !(0. ..=1000.).contains(&f(b, at + 4)) {
        return Err(Error("unsupported collision damage threshold"));
    }
    let threshold = (mass as f64 * f(b, at) as f64) as f32;
    let mut next = Output {
        state: input.clone(),
        random_state,
        applied: false,
    };
    if impact <= threshold {
        return Ok(next);
    }
    let divisor = (mass as f64 * f(b, at + 4) as f64) as f32;
    if divisor <= 0. {
        return Err(Error("invalid active collision damage divisor"));
    }
    let impact = (impact / divisor).min(1.);
    let delta: [f32; 4] = std::array::from_fn(|j| point[j] - f(p, 0x200 + j * 4));
    let mut local = [0.; 4];
    for (j, slot) in local.iter_mut().enumerate().take(3) {
        let row = 0x1d0 + j * 16;
        *slot = (delta[1] * f(p, row + 4) + delta[0] * f(p, row))
            + (delta[3] * f(p, row + 12) + delta[2] * f(p, row + 8));
    }
    local[3] = 1.;
    let mut mutable = b.to_vec();
    let mut apply =
        |level: usize, box_at: usize, coefficients: usize, gain: f32| -> Result<(), Error> {
            let flag = (mutable[(box_at + 32) / 4] & 255) as u8;
            if flag > 1 {
                return Err(Error("invalid collision damage region enable"));
            }
            let result = damage_region::calculate(damage_region::Input {
                previous: f(&mutable, level),
                point: local,
                impact,
                coefficients: std::array::from_fn(|j| f(&mutable, coefficients + j * 4)),
                gain,
                center: std::array::from_fn(|j| f(&mutable, box_at + j * 4)),
                half: std::array::from_fn(|j| f(&mutable, box_at + 16 + j * 4)),
                enabled: flag != 0,
                radius: f(&mutable, 0x370),
                factors: [f(&mutable, 0x374), f(&mutable, 0x378)],
                random_state: next.random_state,
            })?;
            mutable[level / 4] = result.level.to_bits();
            next.random_state = result.random_state;
            Ok(())
        };
    for i in 0..4 {
        apply(0x28 + i * 4, 0x60 + i * 48, 0x240, main_gain)?;
    }
    apply(0, 0x150, 0x218, main_gain)?;
    if !wheel_mode && car[0x104d / 4] >> 8 & 255 != 0 {
        apply(4, 0x180, 0x22c, secondary_gain)?;
    }
    if !wheel_mode {
        apply(8, 0x1b0, 0x304, main_gain)?;
    }
    apply(0x10, 0x1e0, 0x32c, main_gain)?;
    if f(b, 0x42c) > 0. {
        apply(0x14, 0x120, 0x340, main_gain)?;
    }
    vehicle_handling::effects(&mut mutable, queries, true)?;
    next.state.vehicle[m / 4..(m + 0x460) / 4].copy_from_slice(&mutable);
    next.applied = true;
    Ok(next)
}
