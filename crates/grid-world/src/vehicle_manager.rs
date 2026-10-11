//! Single registered car through the manager's pre-job and post-job boundaries.
//! The owned job runs synchronously; worker-pool scheduling remains external.
pub use crate::vehicle_job::{Output, State};
use crate::{
    Error, contact_ray, spawn, suspension_prepare::Contact, vehicle_contact_force::Material,
    vehicle_force, vehicle_handling, vehicle_job,
};

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (8 * (o % 4))) as u8
}
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        b[1] * a[0] - a[1] * b[0],
        0.,
    ]
}
#[allow(clippy::if_same_then_else)] // Match the two original SSE operand orders.
fn norm(a: [f32; 4], square_first: bool) -> Result<[f32; 4], Error> {
    let square = (a[1] * a[1] + a[0] * a[0]) + (a[3] * a[3] + a[2] * a[2]);
    if !square.is_normal() || square <= 0. {
        return Err(Error("invalid manager basis"));
    }
    let seed = spawn::reference_seed(square, false)?;
    let product = if square_first {
        square * seed
    } else {
        seed * square
    };
    let reciprocal = (1. - product * seed) * (seed * 0.5) + seed;
    let out = a.map(|x| reciprocal * x);
    if out.iter().any(|x| !x.is_finite()) {
        return Err(Error("nonfinite manager basis"));
    }
    Ok(out)
}
fn finish(b: &mut [u32]) -> Result<(), Error> {
    if byte(b, 0x1ee0) != 0
        && b[0xdc8 / 4] != 0
        && byte(b, 0x22e4) == 0
        && (0..4).any(|i| {
            let w = 0x1990 + i * 0x150;
            f(b, w + 0x40) > f(b, w + 0x50) && b[(w + 0xc4) / 4] != 0
        })
    {
        return Err(Error("unsupported manager wheel constraint"));
    }
    let v = |o| std::array::from_fn(|i| f(b, o + i * 4));
    let forward = norm(v(0x1600), false)?;
    let up = norm(cross(forward, v(0x15e0)), false)?;
    let side = norm(cross(up, forward), true)?;
    for (o, row) in [(0x15e0, side), (0x15f0, up), (0x1600, forward)] {
        for (i, x) in row.into_iter().enumerate() {
            b[o / 4 + i] = x.to_bits();
        }
    }
    b.copy_within(0x15e0 / 4..0x1620 / 4, 0xb40 / 4);
    b.copy_within(0x15c0 / 4..0x15e0 / 4, 0xb80 / 4);
    Ok(())
}

/// One-car manager interval, including unchanged activation, healthy queries,
/// orientation repair and history copies. A late failure preserves all input.
#[allow(clippy::too_many_arguments)] // Same owned query/contact inputs as the job.
pub fn calculate(
    input: &State,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<Output, Error> {
    let b = &input.owner.vehicle;
    if b.len() != crate::vehicle_publication::VEHICLE_WORDS
        || b[..input.frame.body.len()] != input.frame.body
    {
        return Err(Error("inconsistent manager ownership"));
    }
    if [0xcd8, 0xda8, 0xc0c, 0xd40]
        .into_iter()
        .any(|o| b[o / 4] != 0)
        || (0x248..0x24c).any(|o| byte(b, o) > 1)
        || (byte(b, 0x24b) != 0
            && byte(b, 0x249) != (byte(b, 0x248) & u8::from(byte(b, 0x24a) == 0)))
        || b[0xdc8 / 4] > 1
        || byte(b, 0x1ee0) > 1
    {
        return Err(Error("unsupported manager ownership or activation"));
    }
    let gear = b[0x196c / 4] as usize;
    if gear >= 10
        || !f(b, 0x1648).is_finite()
        || f(b, 0x1648).abs() > 1e10
        || !f(b, 0x18f8 + gear * 4).is_finite()
        || (gear != 9
            && f(b, 0x18f8 + gear * 4) == 0.
            && (!f(b, 0x1894).is_finite()
                || !f(b, 0x17a4).is_finite()
                || f(b, 0x17a4) <= 0.
                || !(f(b, 0x1894) / f(b, 0x17a4)).is_finite()))
    {
        return Err(Error("invalid single-car manager factor"));
    }
    let mut next = input.clone();
    // A single registered car has no inter-car slipstream contributions.
    next.owner.vehicle[0x21c4 / 4] = 1f32.to_bits();
    next.owner.vehicle[0x248 / 4] &= 0x00ff_ffff;
    next.frame
        .body
        .copy_from_slice(&next.owner.vehicle[..vehicle_force::BODY_WORDS]);
    let mut out = vehicle_job::calculate_with_queries(
        &next,
        pair,
        delta,
        gravity,
        handling_queries,
        query,
        material,
    )?;
    finish(&mut out.state.owner.vehicle)?;
    out.state
        .frame
        .body
        .copy_from_slice(&out.state.owner.vehicle[..vehicle_force::BODY_WORDS]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_history_is_published_after_repair() {
        let mut b = vec![0; crate::vehicle_publication::VEHICLE_WORDS];
        for (o, x) in [
            (0x15e0, 2.),
            (0x1608, 3.),
            (0x161c, 1.),
            (0x15c0, 5.),
            (0x15d4, -2.),
        ] {
            b[o / 4] = f32::to_bits(x);
        }
        finish(&mut b).unwrap();
        assert_eq!(&b[0xb40 / 4..0xb80 / 4], &b[0x15e0 / 4..0x1620 / 4]);
        assert_eq!(&b[0xb80 / 4..0xba0 / 4], &b[0x15c0 / 4..0x15e0 / 4]);
        for o in [0x15e0, 0x15f4, 0x1608] {
            assert!((f(&b, o) - 1.).abs() < 1e-6);
        }
        b[0x1608 / 4] = 0;
        assert_eq!(finish(&mut b).unwrap_err().0, "invalid manager basis");
    }
    #[test]
    fn late_basis_failure_preserves_complete_manager_input() {
        let mut vehicle = vec![0; crate::vehicle_publication::VEHICLE_WORDS];
        let mut physical = [0; crate::vehicle_publication::PHYSICAL_WORDS];
        for o in [0xfa8, 0xfac, 0xfb0, 0xfb4, 0x1560, 0x17a4] {
            vehicle[o / 4] = 1f32.to_bits();
        }
        for i in 0..4 {
            physical[0x1d0 / 4 + i * 5] = 1f32.to_bits();
        }
        vehicle[0x1620 / 4] = 4;
        vehicle[0xba0 / 4] = 2;
        vehicle[0x20fb4 / 4] = 1;
        vehicle[0xb10 / 4] = 1 << 24;
        vehicle[0xb14 / 4] = 1;
        vehicle[0x9a0 / 4] = 7;
        vehicle[0xc798 / 4] = 0.1f32.to_bits();
        vehicle[0x248 / 4] = 0x01000101;
        let m = vehicle_handling::MUTABLE;
        for (o, x) in [
            (0x274, 0.65f32),
            (0x278, 0.95),
            (0x368, 0.8),
            (0x430, 1.),
            (0x434, 1.),
            (0x438, 1.),
        ] {
            vehicle[(m + o) / 4] = x.to_bits();
        }
        for i in 0..4 {
            vehicle[(m + 0x18 + i * 4) / 4] = 0.95f32.to_bits();
        }
        vehicle[(m + 0x444) / 4] = 7;
        let mut input = State {
            frame: vehicle_force::Frame {
                body: vehicle[..vehicle_force::BODY_WORDS].try_into().unwrap(),
                view: [0; vehicle_force::VIEW_WORDS],
                solver: [0; 884],
                driven: vec![0, 1],
                material_multipliers: [None; 4],
                diagnostics: [0.; 8],
            },
            owner: crate::vehicle_publication::State { vehicle, physical },
        };
        let query = |_, _| panic!("zero driver must not query contacts");
        let good = calculate(&input, [0, 1], 0., 9.81, None, query, |_| Ok(None)).unwrap();
        assert_eq!(good.state.owner.vehicle[0x248 / 4], 0x00000101);
        assert_eq!(good.state.owner.vehicle[0x21c4 / 4], 1f32.to_bits());
        assert_ne!(
            good.state.owner.physical[0x3f0 / 4],
            input.owner.physical[0x3f0 / 4]
        );
        input.owner.physical[0x1f8 / 4] = 0;
        let before = input.clone();
        assert_eq!(
            calculate(&input, [0, 1], 0., 9.81, None, query, |_| Ok(None))
                .err()
                .unwrap()
                .0,
            "invalid manager basis"
        );
        assert_eq!(input.owner.vehicle, before.owner.vehicle);
        assert_eq!(input.owner.physical, before.owner.physical);
        assert_eq!(format!("{:?}", input.frame), format!("{:?}", before.frame));
    }
}
