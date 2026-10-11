//! Selected outer vehicle step: temporary frame, whole driver and motion assistance.
//! Physical synchronization/publication and world dispatch remain external.
use crate::{
    Error, contact_ray, suspension_prepare::Contact, vehicle_contact_force::Material,
    vehicle_force::Frame,
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
fn v(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
fn store(b: &mut [u32], o: usize, x: [f32; 4]) {
    for (i, x) in x.into_iter().enumerate() {
        set(b, o + i * 4, x);
    }
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn speed(a: [f32; 4]) -> Result<f32, Error> {
    let square = dot(a, a);
    if !square.is_finite() {
        return Err(Error("invalid outer motion magnitude"));
    }
    if square == 0. {
        return Ok(0.);
    }
    let seed = crate::spawn::reference_seed(square, false)?;
    let product = seed * square;
    Ok((1. - product * seed) * (product * 0.5) + product)
}
fn validate(b: &[u32], delta: f32) -> Result<(), Error> {
    if b.len() != crate::vehicle_force::BODY_WORDS
        || !delta.is_finite()
        || !(0. ..=0.1).contains(&delta)
        || [
            0x2e8, 0xe1c, 0xe20, 0x11f0, 0x1224, 0x1228, 0x122c, 0x1230, 0x1234, 0x1238, 0x1648,
        ]
        .into_iter()
        .any(|o| !f(b, o).is_finite())
        || (0x15c0..0x1620).step_by(4).any(|o| !f(b, o).is_finite())
    {
        return Err(Error("invalid outer vehicle step"));
    }
    if (f(b, 0x122c) > 0. || f(b, 0x1230) > 0.) && f(b, 0x1224) == f(b, 0x1228) {
        return Err(Error("invalid outer angle span"));
    }
    Ok(())
}
/// Motion assistance after the driver, with the pose captured before its call.
/// Also accepts zero delta, where the original outer wrapper skips the driver.
#[allow(clippy::assign_op_pattern, clippy::neg_multiply)] // Observed operand order.
fn finish(b: &mut [u32], previous: [f32; 4], delta: f32) -> Result<(), Error> {
    let linear = v(b, 0x15c0);
    let up = v(b, 0x15f0);
    if f(b, 0x122c) > 0. || f(b, 0x1230) > 0. {
        let low = f(b, 0x1234);
        let span = f(b, 0x1238) - low;
        let factor = if span > 0. {
            ((f(b, 0x1648) - low) / span).clamp(0., 1.)
        } else if f(b, 0x1648) > low {
            1.
        } else {
            0.
        };
        let contacts = (0..4)
            .filter(|i| b[(0x1990 + i * 0x150 + 0xc0) / 4] != 0)
            .count();
        if factor > 0. && contacts != 0 {
            let factor = ((contacts as f32) * factor) * 0.25;
            let square = dot(linear, linear);
            if !square.is_finite() || square <= 0. {
                return Err(Error("invalid outer angle velocity"));
            }
            let seed = crate::spawn::reference_seed(square, false)?;
            let reciprocal = (1. - (seed * square) * seed) * (seed * 0.5) + seed;
            let normalized = linear.map(|x| reciprocal * x);
            let sideways = dot(normalized, v(b, 0x15e0));
            if !(-1. ..=1.).contains(&sideways) {
                return Err(Error("invalid outer angle domain"));
            }
            let angle = sideways as f64;
            let angle = angle.asin() as f32;
            let signed = -0.0 - angle;
            let amount =
                ((signed.abs() - f(b, 0x1224)) / (f(b, 0x1228) - f(b, 0x1224))).clamp(0., 1.);
            if amount > 0. {
                let mut correction = (f(b, 0x122c) * amount) * factor;
                if signed > 0. {
                    correction = -0.0 - correction;
                }
                let angular = v(b, 0x15d0);
                let damping = ((dot(up, angular) * amount) * factor) * f(b, 0x1230);
                let change = (correction - damping) * delta;
                store(
                    b,
                    0x15d0,
                    std::array::from_fn(|i| change * up[i] + angular[i]),
                );
            }
        }
    }
    let linear_speed = speed(v(b, 0x15c0))?;
    let timer = f(b, 0x2e8);
    if linear_speed >= f32::from_bits(0x3dcccccd) {
        if linear_speed > 5. && timer < 2. {
            set(b, 0x2e8, 2.);
        }
    } else if timer > 0. {
        set(b, 0x2e8, (timer - delta).max(0.));
    } else if speed(v(b, 0x15d0))? < f32::from_bits(0x3dcccccd) && byte(b, 0x1640) != 0 {
        let position = v(b, 0x1610);
        let difference: [f32; 4] = std::array::from_fn(|i| position[i] - previous[i]);
        let movement = dot(difference, up);
        store(
            b,
            0x1610,
            std::array::from_fn(|i| movement * up[i] + previous[i]),
        );
        let linear = v(b, 0x15c0);
        let along = dot(linear, up);
        store(b, 0x15c0, up.map(|x| along * x));
        let angular = v(b, 0x15d0);
        let around = dot(angular, up);
        store(
            b,
            0x15d0,
            std::array::from_fn(|i| angular[i] - around * up[i]),
        );
        for i in 0..4 {
            if b[(0x1990 + i * 0x150 + 0xc0) / 4] != 0 {
                set(b, 0x1990 + i * 0x150 + 0x104, 0.);
            }
        }
    }
    if f(b, 0x11f0) > 0. {
        let linear = v(b, 0x15c0);
        let magnitude = speed(linear)?;
        if magnitude > 0. {
            let contacts = (0..4)
                .filter(|i| b[(0x1990 + i * 0x150 + 0xc8) / 4] != 0)
                .count() as f32;
            // Caller computes this complete coefficient before the SIMD block.
            let amount = (contacts * 0.25) * ((f(b, 0xe20) * f(b, 0x11f0)) * delta);
            let scale = (magnitude - amount) / magnitude;
            if !amount.is_finite() || !scale.is_finite() {
                return Err(Error("nonfinite outer braking coefficient"));
            }
            let scale = scale.max(0.);
            store(b, 0x15c0, linear.map(|x| scale * x));
        }
    }
    if [0x2e8].into_iter().any(|o| !f(b, o).is_finite())
        || (0x15c0..0x1620).step_by(4).any(|o| !f(b, o).is_finite())
    {
        return Err(Error("nonfinite outer vehicle output"));
    }
    Ok(())
}

pub fn apply(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    mut material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    validate(&frame.body, delta)?;
    let mut next = frame.clone();
    let previous = v(&next.body, 0x1610);
    let inactive = byte(&next.body, 0xe38) != 0
        || (f(&next.body, 0x1648) < 0.5
            && ((f(&next.body, 0xe1c) == 0. && f(&next.body, 0xe20) == 0.)
                || next.body[0x196c / 4] == 0));
    next.body[0x1640 / 4] = (next.body[0x1640 / 4] & !255) | u32::from(inactive);
    let owner = next.view[0];
    next.view.fill(0);
    next.view[0] = owner;
    next.solver.fill(0);
    for o in (0x298..0x2a8).step_by(4) {
        next.view[o / 4] = 1f32.to_bits();
    }
    let result = if delta > 0. {
        crate::vehicle_contact_update::apply_driver(
            &mut next, pair, delta, gravity, query, material,
        )?
    } else {
        let mut result = [None; 4];
        for (i, m) in result.iter_mut().enumerate() {
            let token = next.body[(0x1990 + i * 0x150 + 0xc0) / 4];
            if token != 0 {
                *m = Some(material(token)?.ok_or(Error("missing outer wheel mechanics"))?);
            }
        }
        result
    };
    finish(&mut next.body, previous, delta)?;
    *frame = next;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_delta_skips_queries_and_settles_only_selected_motion() {
        let mut frame = crate::vehicle_contact_force::tests::failure_frame();
        store(&mut frame.body, 0x15c0, [0.03, 0.02, 0.01, 0.]);
        store(&mut frame.body, 0x15d0, [0.01, 0.02, 0.03, 0.]);
        store(&mut frame.body, 0x15f0, [0., 1., 0., 0.]);
        store(&mut frame.body, 0x1610, [7., 8., 9., 1.]);
        set(&mut frame.body, 0x1648, 0.03);
        let result = apply(
            &mut frame,
            [2, 3],
            0.,
            9.81,
            |_, _| panic!("zero delta must not query"),
            |_| panic!("no material token"),
        )
        .unwrap();
        assert!(result.iter().all(Option::is_none));
        assert_eq!(byte(&frame.body, 0x1640), 1);
        assert_eq!(v(&frame.body, 0x1610), [7., 8., 9., 1.]);
        assert_eq!(v(&frame.body, 0x15c0), [0., 0.02, 0., 0.]);
        assert_eq!(v(&frame.body, 0x15d0), [0.01, 0., 0.03, 0.]);
    }
    #[test]
    fn late_braking_overflow_rolls_back_temporary_frame_and_all_state() {
        let mut frame = crate::vehicle_contact_force::tests::failure_frame();
        store(&mut frame.body, 0x15c0, [1., 2., 3., 0.]);
        set(&mut frame.body, 0x11f0, f32::MAX);
        set(&mut frame.body, 0xe20, f32::MAX);
        frame.view[0x10 / 4] = 0xdeadbeef;
        frame.solver[60] = 0x12345678;
        let before = frame.clone();
        let error = apply(
            &mut frame,
            [2, 3],
            0.,
            9.81,
            |_, _| panic!("zero delta must not query"),
            |_| panic!("no material token"),
        )
        .unwrap_err();
        assert_eq!(error.0, "nonfinite outer braking coefficient");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.driven, before.driven);
        assert_eq!(frame.diagnostics, before.diagnostics);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
    }
}
