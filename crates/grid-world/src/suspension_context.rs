//! Source caster/camber axes and history reset on an owned four-wheel context.
//! The caller supplies copied source properties and raw local wheel positions.
use crate::{Error, spawn, vehicle_force};

/// Whole selected 9C10B0 -> 9983E0. Context9A4 is the canonical owned self
/// identity zero; wheel backlinks are relative offsets, never machine addresses.
pub fn initialize(
    input: &[u32; vehicle_force::BODY_WORDS],
) -> Result<[u32; vehicle_force::BODY_WORDS], Error> {
    let f = |o: usize| f32::from_bits(input[o / 4]);
    if input[0x9a4 / 4] != 0
        || input[0x9a8 / 4] != 4
        || [0x103c, 0x1040, 0x1044, 0x1048]
            .iter()
            .any(|&o| !f(o).is_finite() || f(o).abs() > 1.)
        || (0x14e0..0x1520).step_by(4).any(|o| !f(o).is_finite())
    {
        return Err(Error("invalid source suspension context"));
    }
    let mut next = *input;
    next[0x9a0 / 4] &= !255;
    for i in 0..4 {
        let c = 0x3d0 + i * 0x170;
        for o in [0x68, 0x6c, 0x7c, 0x80, 0x150] {
            next[(c + o) / 4] = 0;
        }
        let front = f(0x14e8 + i * 16) > 0.;
        let caster = f(if front { 0x103c } else { 0x1040 });
        let camber = f(if front { 0x1044 } else { 0x1048 });
        let side = if f(0x14e0 + i * 16) > 0. { 2. } else { -2. };
        let axis = [camber * side, 1., f64::from(caster).tan() as f32, 0.];
        let squared =
            (axis[1] * axis[1] + axis[0] * axis[0]) + (axis[3] * axis[3] + axis[2] * axis[2]);
        let seed = spawn::reference_seed(squared, false)?;
        let inverse = (1. - (squared * seed) * seed) * (seed * 0.5) + seed;
        for (j, value) in axis.into_iter().enumerate() {
            let value = inverse * value;
            if !value.is_finite() {
                return Err(Error("nonfinite source suspension axis"));
            }
            next[(c + 0x50 + j * 4) / 4] = value.to_bits();
        }
        next[(c + 0x178) / 4] = (0x1990 + i * 0x150) as u32;
    }
    Ok(next)
}
