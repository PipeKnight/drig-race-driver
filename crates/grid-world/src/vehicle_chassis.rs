//! Suspension/acceleration chassis-motion matrix, following the force/assist interval.
//! This publishes the separate local chassis matrix, not the physical body pose.
use crate::{Error, spawn, vehicle_force::BODY_WORDS};
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, x: f32) {
    b[o / 4] = x.to_bits();
}
fn v(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[2] * b[2] + a[3] * b[3]) + (a[0] * b[0] + a[1] * b[1])
}
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        b[1] * a[0] - a[1] * b[0],
        0.,
    ]
}
fn norm(a: [f32; 4]) -> Result<[f32; 4], Error> {
    let square = dot(a, a);
    if !square.is_finite() || square <= 0. {
        return Err(Error("invalid chassis normalization"));
    }
    let seed = spawn::reference_seed(square, false)?;
    let reciprocal = (1. - (seed * square) * seed) * (seed * 0.5) + seed;
    let a = a.map(|x| reciprocal * x);
    if a.iter().any(|x| !x.is_finite()) {
        return Err(Error("nonfinite chassis normalization"));
    }
    Ok(a)
}
fn interpolate(b: &[u32], base: usize, phase: f32) -> f32 {
    let index = phase as usize;
    let fraction = phase - index as f32;
    (1. - fraction) * f(b, base + index * 4) + f(b, base + ((index + 1) % 60) * 4) * fraction
}

/// Execute the whole selected chassis-motion path on a copy, then commit.
/// Phase 60 is intentionally retained: the original subtracts only above 60
/// and its first index may therefore read the adjacent owned word.
#[allow(clippy::assign_op_pattern, clippy::neg_multiply)] // Preserve original operand order.
pub fn apply(body: &mut [u32; BODY_WORDS], delta: f32) -> Result<(), Error> {
    if !delta.is_finite() || delta <= 0. || delta > 0.1 || !(1. / delta).is_finite() {
        return Err(Error("invalid chassis delta"));
    }
    let fields = [0x13d0, 0x13d4, 0x1648];
    if fields.into_iter().any(|o| !f(body, o).is_finite())
        || [
            (0x50, 0x24c),
            (0xb20, 0xb30),
            (0x1494, 0x14d0),
            (0x15c0, 0x15d0),
            (0x15e0, 0x1610),
            (0x1630, 0x1640),
        ]
        .into_iter()
        .any(|(a, z)| (a..z).step_by(4).any(|o| !f(body, o).is_finite()))
        || (0..4).any(|i| {
            [0, 8, 0x40]
                .into_iter()
                .any(|o| !f(body, 0x1990 + i * 0x150 + o).is_finite())
        })
        || f(body, 0x14a8) == f(body, 0x14ac)
        || f(body, 0x14c0) == f(body, 0x14c4)
        || f(body, 0x1498) < 0.
        || f(body, 0x149c) < 0.
        || !(f(body, 0x1ae0) - f(body, 0x1990)).is_finite()
        || f(body, 0x1ae0) == f(body, 0x1990)
        || !(f(body, 0x1c38) - f(body, 0x1998)).is_finite()
        || f(body, 0x1c38) == f(body, 0x1998)
    {
        return Err(Error("invalid chassis scalar or span"));
    }
    let mut phases = [0.; 2];
    for (i, (phase, rate)) in [(0x60, 0x14cc), (0x64, 0x14c8)].into_iter().enumerate() {
        let x = (f(body, rate) * delta.abs()) * 20. + f(body, phase);
        phases[i] = if x > 60. { x - 60. } else { x };
        if !phases[i].is_finite() || !(0. ..=60.).contains(&phases[i]) {
            return Err(Error("chassis phase exceeds table bound"));
        }
    }
    let mut next = *body;
    let b = &mut next;
    let side = v(b, 0x15e0);
    let forward = v(b, 0x1600);
    let inverse = 1. / delta;
    let blend = (f(b, 0x1494) * delta).clamp(0., 1.);
    let acceleration = std::array::from_fn(|i| {
        blend * ((f(b, 0x15c0 + i * 4) - f(b, 0xb20 + i * 4)) * inverse)
            + (1. - blend) * f(b, 0x50 + i * 4)
    });
    let lateral = dot(acceleration, side).clamp(-0.0 - f(b, 0x1498), f(b, 0x1498));
    let longitudinal = dot(acceleration, forward).clamp(-0.0 - f(b, 0x149c), f(b, 0x149c));
    let side_offset = lateral * f(b, 0x14a0);
    let long_offset = longitudinal * f(b, 0x14a4);
    let offsets: [f32; 4] =
        std::array::from_fn(|i| long_offset * forward[i] + side_offset * side[i]);
    for i in 0..4 {
        set(
            b,
            0x50 + i * 4,
            lateral * side[i] + longitudinal * forward[i],
        );
    }
    set(b, 0x60, phases[0]);
    set(b, 0x64, phases[1]);
    let amount = ((f(b, 0x1648) - f(b, 0x14ac)) / (f(b, 0x14a8) - f(b, 0x14ac))).clamp(0., 1.);
    let amplitude = (f(b, 0x14b0) - f(b, 0x14b4)) * amount + f(b, 0x14b4);
    let table_scale = ((interpolate(b, 0x68, phases[1]) - f(b, 0x14c4))
        / (f(b, 0x14c0) - f(b, 0x14c4)))
    .clamp(0., 1.);
    let noise = [
        (interpolate(b, 0x68, phases[0]) * table_scale) * amplitude,
        0.,
        (interpolate(b, 0x158, phases[0]) * table_scale) * amplitude,
        0.,
    ];
    let left = (f(b, 0x1c70) + f(b, 0x19d0)) * 0.5;
    let right = (f(b, 0x1dc0) + f(b, 0x1b20)) * 0.5;
    let roll = (f(b, 0x14b8) - 1.) * ((left - right) / (f(b, 0x1ae0) - f(b, 0x1990)).abs());
    let front = (f(b, 0x1c70) + f(b, 0x1dc0)) * 0.5;
    let rear = (f(b, 0x19d0) + f(b, 0x1b20)) * 0.5;
    let pitch = ((front - rear) / (f(b, 0x1c38) - f(b, 0x1998)).abs()) * (f(b, 0x14bc) - 1.);
    let up = [0., 1., 0., 0.];
    let normal = std::array::from_fn(|i| {
        (roll * side[i] + ((noise[i] + up[i]) + offsets[i])) + pitch * forward[i]
    });
    let mut normal = norm(normal)?;
    let rear = (f(b, 0x19d0) - f(b, 0x13d4))
        .min(f(b, 0x1b20) - f(b, 0x13d4))
        .max(0.);
    let front = (f(b, 0x1c70) - f(b, 0x13d0))
        .min(f(b, 0x1dc0) - f(b, 0x13d0))
        .max(0.);
    let mut baseline = front.min(rear);
    let front = front - baseline;
    let rear = rear - baseline;
    if front > 0. || rear > 0. {
        let horizontal = norm([forward[0], 0., forward[2], forward[3]])?;
        let perpendicular = [-horizontal[2], 0., horizontal[0], 0.];
        let front_z = f(b, 0x1c38) - f(b, 0x1638);
        let rear_z = f(b, 0x1998) - f(b, 0x1638);
        let (extra, slope, limit_upper) = if front > 0. {
            let extra = ((-0.0 - rear_z) * front) / (front_z - rear_z);
            (extra, (front - extra) * (-1. / front_z), true)
        } else {
            let extra = (rear * front_z) / (front_z - rear_z);
            (extra, (rear - extra) * (-1. / rear_z), false)
        };
        baseline = extra + baseline;
        let horizontal_component = dot(normal, horizontal);
        let vertical = dot(normal, up);
        let target = vertical * slope;
        if (limit_upper && horizontal_component > target)
            || (!limit_upper && target > horizontal_component)
        {
            let lateral = dot(normal, perpendicular);
            normal = norm(std::array::from_fn(|i| {
                (lateral * perpendicular[i] + vertical * up[i]) + target * horizontal[i]
            }))?;
        }
    }
    normal[3] = 0.;
    let mut chassis_side = norm(cross(normal, [0., 0., 1., 0.]))?;
    let chassis_forward = cross(chassis_side, normal);
    if chassis_side[0] < 0. {
        chassis_side = chassis_side.map(|x| -1. * x);
    }
    let mut position = normal.map(|x| baseline * x);
    position[3] = 1.;
    for (j, row) in [chassis_side, normal, chassis_forward, position]
        .into_iter()
        .enumerate()
    {
        for (i, x) in row.into_iter().enumerate() {
            set(b, 0x10 + j * 16 + i * 4, x);
        }
    }
    if (0x10..0x68).step_by(4).any(|o| !f(b, o).is_finite()) {
        return Err(Error("nonfinite chassis output"));
    }
    *body = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> [u32; BODY_WORDS] {
        let mut b = [0; BODY_WORDS];
        for (o, x) in [
            (0x1494, 100f32),
            (0x1498, 10.),
            (0x149c, 10.),
            (0x14a8, 1.),
            (0x14c0, 1.),
            (0x15e0, 1.),
            (0x15f4, 1.),
            (0x1608, 1.),
        ] {
            set(&mut b, o, x);
        }
        for i in 0..4 {
            set(
                &mut b,
                0x1990 + i * 0x150,
                if i % 2 == 0 { -0.8 } else { 0.8 },
            );
            set(&mut b, 0x1998 + i * 0x150, if i < 2 { -1.3 } else { 1.3 });
        }
        b
    }
    #[test]
    fn local_matrix_publication_preserves_physical_pose() {
        let mut b = fixture();
        let physical = b[0x15c0 / 4..0x1620 / 4].to_vec();
        apply(&mut b, 0.01).unwrap();
        assert_eq!(b[0x4c / 4], 1f32.to_bits());
        assert_eq!(b[0x2c / 4], 0);
        assert_eq!(b[0x3c / 4], 0);
        assert_eq!(b[0x15c0 / 4..0x1620 / 4], physical);
    }
    #[test]
    fn normalization_overflow_preserves_filtered_state_and_matrix() {
        let mut b = fixture();
        set(&mut b, 0x14a0, f32::MAX);
        set(&mut b, 0x15c0, 1e30);
        set(&mut b, 0x60, 0.2);
        let before = b;
        assert_eq!(
            apply(&mut b, 0.01).unwrap_err().0,
            "invalid chassis normalization"
        );
        assert_eq!(b, before);
    }
}
