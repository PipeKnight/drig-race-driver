//! Complete supplied-bounds 9A2A70/989490/98A7D0 layout and node reset.
//! Source object construction, node limits/markers and edges remain separate.
use crate::{Error, spawn::reference_seed};

pub const WORDS: usize = 0xa500 / 4;

#[derive(Clone, Copy)]
pub struct Bounds {
    /// Only XYZ is consumed; original reduction retains its own W=1.
    pub minimum: [f32; 4],
    pub maximum: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct Parameters {
    pub margin: f32,
    pub spacing_factor: f32,
    pub spacing_growth: f32,
    pub fallback_xy: f32,
    pub fallback_z: f32,
    pub flag_a4b0: u8,
}

pub struct Output {
    pub descriptor: Vec<u32>,
    /// Diagnostic count from the independent implementation, not a live clock.
    pub spacing_iterations: usize,
}

fn store(b: &mut [u32], o: usize, value: f32) {
    b[o / 4] = value.to_bits();
}
fn byte(b: &mut [u32], o: usize, value: u8) {
    let shift = (o % 4) * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | (u32::from(value) << shift);
}

/// Own every byte. The settings link is an opaque copied token, never followed.
/// Bounds/iteration limits are explicit admission rules, not original errors.
pub fn calculate(
    descriptor: &[u32],
    bounds: &[Bounds],
    settings_token: u32,
    p: Parameters,
) -> Result<Output, Error> {
    if descriptor.len() != WORDS
        || bounds.len() > 32
        || !p.margin.is_finite()
        || !(0. ..=10.).contains(&p.margin)
        || !p.spacing_factor.is_finite()
        || !(0.005..=1.).contains(&p.spacing_factor)
        || !p.spacing_growth.is_finite()
        || !(1.001..=2.).contains(&p.spacing_growth)
        || [p.fallback_xy, p.fallback_z]
            .iter()
            .any(|v| !v.is_finite() || !(-100. ..=-0.001).contains(v))
        || bounds.iter().any(|b| {
            b.minimum[..3]
                .iter()
                .chain(&b.maximum[..3])
                .any(|v| !v.is_finite() || v.abs() > 1000.)
        })
    {
        return Err(Error("invalid deformation layout input"));
    }
    let mut lower = [f32::MAX, f32::MAX, f32::MAX, 1.];
    let mut upper = [-f32::MAX, -f32::MAX, -f32::MAX, 1.];
    for b in bounds {
        for axis in 0..3 {
            // Strict comparisons preserve the first equal signed-zero word.
            if b.minimum[axis] < lower[axis] {
                lower[axis] = b.minimum[axis];
            }
            if upper[axis] < b.maximum[axis] {
                upper[axis] = b.maximum[axis];
            }
        }
    }
    for axis in 0..3 {
        lower[axis] -= p.margin;
        upper[axis] += p.margin;
    }
    lower[3] -= 0.;
    upper[3] += 0.;
    // The original broadcasts the X comparison before MOVMSKPS.
    if upper[0] < lower[0] {
        lower = [p.fallback_xy, p.fallback_xy, p.fallback_z, 0.];
        upper = lower.map(|v| p.fallback_xy * v);
    }
    let center: [f32; 4] = std::array::from_fn(|i| 0.5 * (upper[i] + lower[i]));
    let span: [f32; 4] = std::array::from_fn(|i| upper[i] - lower[i]);
    if span.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err(Error("invalid deformation layout extent"));
    }
    let square = (span[1] * span[1] + span[0] * span[0]) + (span[3] * span[3] + span[2] * span[2]);
    if !square.is_normal() || square <= 0. {
        return Err(Error("degenerate deformation layout extent"));
    }
    let seed = reference_seed(square, false)?;
    let product = square * seed;
    let length = (1. - product * seed) * (product * 0.5) + product;
    let mut spacing = length * p.spacing_factor;
    let mut dimensions = [0usize; 3];
    let mut iterations = 0;
    loop {
        iterations += 1;
        if iterations > 4096 {
            return Err(Error("deformation layout iteration limit"));
        }
        // Even an already small grid gets one mandatory growth step.
        spacing *= p.spacing_growth;
        if !spacing.is_normal() || spacing <= 0. {
            return Err(Error("invalid deformation layout spacing"));
        }
        let inverse = 1. / spacing;
        for axis in 0..3 {
            let ratio = inverse * span[axis];
            if !ratio.is_finite() || !(0. ..=1000.).contains(&ratio) {
                return Err(Error("deformation layout conversion limit"));
            }
            dimensions[axis] = ratio.trunc() as usize + 2;
        }
        if dimensions.iter().product::<usize>() <= 400 {
            break;
        }
    }
    for axis in 0..3 {
        lower[axis] = center[axis] - ((dimensions[axis] - 1) as f32 * spacing) * 0.5;
    }
    let mut next = descriptor.to_vec();
    byte(&mut next, 0xa4b0, p.flag_a4b0);
    byte(&mut next, 0xa4ad, 1);
    next[0xa4a0 / 4] = u32::MAX;
    next[0xa4f8 / 4] = settings_token;
    for (axis, dimension) in dimensions.into_iter().enumerate() {
        next[0xa414 / 4 + axis] = dimension as u32;
    }
    store(&mut next, 0xa420, spacing);
    for (axis, value) in lower.into_iter().enumerate() {
        store(&mut next, 0xa430 + 4 * axis, value);
    }
    let [nx, ny, nz] = dimensions;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let index = (z * ny + y) * nx + x;
                let at = 0x2580 + index * 80;
                let position = [
                    x as f32 * spacing + lower[0],
                    y as f32 * spacing + lower[1],
                    z as f32 * spacing + lower[2],
                    0. + lower[3],
                ];
                for (lane, value) in position.into_iter().enumerate() {
                    store(&mut next, at + 4 * lane, value);
                    store(&mut next, at + 16 + 4 * lane, value);
                }
                next[(at + 0x30) / 4..(at + 0x40) / 4].fill(0);
                byte(&mut next, 0xa280 + index, 255);
            }
        }
    }
    next[0xa440 / 4..0xa450 / 4].fill(0);
    Ok(Output {
        descriptor: next,
        spacing_iterations: iterations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_layout_uses_fallback_and_leaves_markers_edges_and_padding_owned() {
        let initial = vec![0x12345678; WORDS];
        let result = calculate(
            &initial,
            &[],
            0xdeadbeef,
            Parameters {
                margin: 0.,
                spacing_factor: 0.25,
                spacing_growth: 1.5,
                fallback_xy: -1.,
                fallback_z: -3.,
                flag_a4b0: 7,
            },
        )
        .unwrap();
        let d = &result.descriptor;
        assert_eq!(d[0xa4f8 / 4], 0xdeadbeef);
        assert_eq!(d[0xa4a0 / 4], u32::MAX);
        assert!(result.spacing_iterations >= 1);
        assert_eq!(d[0x25c0 / 4], initial[0x25c0 / 4]);
        assert_eq!(d[0xa410 / 4], initial[0xa410 / 4]);
        assert_eq!(d[0], initial[0]);
        assert_eq!(d[(0x2580 + 399 * 80) / 4], initial[(0x2580 + 399 * 80) / 4]);
        assert!(initial.iter().all(|w| *w == 0x12345678));
    }
}
