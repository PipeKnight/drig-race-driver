//! Whole selected A03220 after collision/wheel rebuilding, with an owned empty
//! graphical part collection and bounded light/wheel presentation records.
use crate::{
    Error, deformation_grid,
    spawn::reference_seed,
    vehicle_publication::{self, State, VEHICLE_WORDS},
};
fn f(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
fn byte(b: &[u32], at: usize) -> u8 {
    (b[at / 4] >> (at % 4 * 8)) as u8
}
fn put_byte(b: &mut [u32], at: usize, v: u8) {
    let shift = at % 4 * 8;
    b[at / 4] = (b[at / 4] & !(255 << shift)) | (u32::from(v) << shift);
}
fn vector(b: &[u32], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, at + i * 4))
}
fn squared(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
    (d[1] * d[1] + d[0] * d[0]) + d[2] * d[2]
}
pub fn calculate(input: &State, delta: f32) -> Result<State, Error> {
    let b = &input.vehicle;
    if b.len() != VEHICLE_WORDS || !delta.is_finite() || !(0. ..=0.1).contains(&delta) {
        return Err(Error("invalid deformation publication framing"));
    }
    let independent = b[0x1554 / 4] == 0;
    if b[0xdb0 / 4] != 0
        || b[0x1620 / 4] != 4
        || b[0xdb8 / 4..0xdc8 / 4].iter().any(|&x| x != 0)
        || b[0xdd4 / 4] != 0
        || byte(b, 4) > 1
        || byte(b, 0xc7f2) != 0
        || b[0xba0 / 4] != if independent { 2 } else { 1 }
        || byte(b, 0x20fb4) != u8::from(independent)
        || [0xc798, 0xc790].iter().any(|&at| !f(b, at).is_finite())
        || [0x15c0, 0x15d0]
            .iter()
            .any(|&at| (0..3).any(|i| !f(b, at + i * 4).is_finite()))
        || b[0xc8a4 / 4] != 0
        || b[0xd004 / 4] != 0
    {
        return Err(Error(
            "unsupported deformation publication ownership or pending rebuild",
        ));
    }
    let mut next = vehicle_publication::update_without_rebuild(input, delta);
    if b[0xc7a0 / 4] == 1 {
        next.vehicle[0xcdfc / 4] = 1f32.to_bits();
        next.vehicle[0xcdf4 / 4] = 1f32.to_bits();
    }
    if byte(b, 4) == 0 {
        return Ok(next);
    }
    if b[0xc7f8 / 4] != 0xd240
        || !f(b, 0xee40).is_finite()
        || f(b, 0xee40).abs() > 1000.
        || (0..12).any(|i| {
            b[(0xcc10 + i * 32) / 4] > 2
                || vector(b, 0xcc20 + i * 32)
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
        })
        || (0..4).any(|i| {
            !f(b, 0xcd90 + i * 4).is_finite()
                || f(b, 0xcd90 + i * 4) <= 0.
                || vector(b, 0xcda0 + i * 16)
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1e6)
        })
    {
        return Err(Error("invalid owned deformation view"));
    }
    if byte(b, 0xc7ad) == 0 {
        for i in 0..4 {
            put_byte(&mut next.vehicle, 0xcde0 + i, 255);
        }
        return Ok(next);
    }
    let dimensions: [usize; 3] = std::array::from_fn(|i| b[0xc714 / 4 + i] as usize);
    if dimensions.iter().any(|d| !(2..=16).contains(d))
        || dimensions.iter().product::<usize>() > 400
    {
        return Err(Error("invalid published deformation grid"));
    }
    let positions: Vec<_> = (0..dimensions.iter().product::<usize>())
        .map(|i| vector(b, 0x4880 + i * 80))
        .collect();
    let grid = deformation_grid::Grid {
        dimensions,
        spacing: f(b, 0xc720),
        minimum: vector(b, 0xc730),
        positions: &positions,
    };
    for i in 0..12 {
        let at = 0xcc10 + i * 32;
        if b[at / 4] == 0 {
            continue;
        }
        let original = vector(b, at + 16);
        let mut point = original;
        point[3] = 0.;
        let sampled = grid.sample(point)?;
        let local = std::array::from_fn(|a| sampled[a] - f(b, 0xc740 + a * 4));
        if squared(original, local) > f(b, 0xee40) * f(b, 0xee40) {
            next.vehicle[at / 4] = 2;
        }
    }
    for i in 0..4 {
        let original = vector(b, 0xcda0 + i * 16);
        let mut point = original;
        point[3] = 0.;
        let sampled = grid.sample(point)?;
        let square = squared(sampled, original);
        let root = if square == 0. {
            0.
        } else {
            let seed = reference_seed(square, false)?;
            let product = seed * square;
            (1. - product * seed) * (product * 0.5) + product
        };
        let at = 0xcd90 + if i < 2 { 8 } else { 0 };
        let amount = (root - f(b, at)) * (255. / f(b, at + 4));
        if !amount.is_finite() || amount < i32::MIN as f32 || amount >= 2147483648. {
            return Err(Error("published wheel damage conversion exceeds i32"));
        }
        let level = 255i32.wrapping_sub(amount as i32).clamp(0, 255) as u8;
        put_byte(&mut next.vehicle, 0xcde0 + i, level);
    }
    Ok(next)
}

/// Boundary after the caller has successfully rebuilt the actual collision
/// geometry. Stage the original dirty flag/body4 transition and whole wheel
/// update, then the reached publication. This does not implement A01960.
pub fn after_geometry_rebuild(input: &State, delta: f32) -> Result<State, Error> {
    if input.vehicle.len() != VEHICLE_WORDS
        || byte(&input.vehicle, 0xc7f2) > 1
        || byte(&input.vehicle, 4) > 1
    {
        return Err(Error("invalid completed deformation rebuild boundary"));
    }
    let mut next = input.clone();
    if byte(&input.vehicle, 0xc7f2) != 0 {
        put_byte(&mut next.vehicle, 0xc7f2, 0);
        put_byte(&mut next.vehicle, 4, 1);
        next.vehicle = crate::wheel_deformation_state::calculate(&next.vehicle)?;
    }
    calculate(&next, delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_view_failure_preserves_earlier_decay_mode_and_velocity_state() {
        let mut state = State {
            vehicle: vec![0; VEHICLE_WORDS],
            physical: [0; 0x424 / 4],
            damage_random_state: Some(17),
        };
        let b = &mut state.vehicle;
        b[0x1620 / 4] = 4;
        b[0xba0 / 4] = 2;
        b[0x20fb4 / 4] = 1;
        b[1] = 1;
        b[0xc7a0 / 4] = 1;
        b[0xc7f8 / 4] = 0xd240;
        for (at, value) in [
            (0xc798, 0.1f32),
            (0xc790, 0.5),
            (0xcdf4, 0.3),
            (0xcdfc, 0.4),
            (0xee40, 0.1),
        ] {
            b[at / 4] = value.to_bits();
        }
        for at in [0xcd90, 0xcd94, 0xcd98] {
            b[at / 4] = 0.1f32.to_bits();
        }
        b[0x15cc / 4] = 0x7fc01234;
        let before = state.clone();
        assert!(calculate(&state, 0.016).is_err());
        assert_eq!(state.vehicle, before.vehicle);
        assert_eq!(state.physical, before.physical);
        assert_eq!(state.damage_random_state, Some(17));
    }
}
