//! Wheel cast endpoints for supplied geometry; world query/material handling separate.
use crate::Error;

/// Increase contact-normal alignment with the supplied suspension axis. Retains
/// the original approximate SSE inverse-square-root refinement. 1,024 iterations
/// is an authored diagnostic bound; the original loop has no such guard.
pub fn regularize(mut normal: [f32; 4], axis: [f32; 4]) -> Result<[f32; 4], Error> {
    if normal.iter().chain(&axis).any(|v| !v.is_finite()) {
        return Err(Error("invalid contact normal inputs"));
    }
    for _ in 0..1024 {
        let dot = (normal[1] * axis[1] + normal[0] * axis[0])
            + (normal[3] * axis[3] + normal[2] * axis[2]);
        if dot >= f32::from_bits(0x3e31_26e9) {
            return Ok(normal);
        }
        let updated: [f32; 4] =
            std::array::from_fn(|i| f32::from_bits(0x3d4c_cccd) * axis[i] + normal[i]);
        let squared = (updated[1] * updated[1] + updated[0] * updated[0])
            + (updated[3] * updated[3] + updated[2] * updated[2]);
        let seed = crate::spawn::reference_seed(squared, false)?;
        let inverse = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
        normal = updated.map(|v| inverse * v);
        if normal.iter().any(|v| !v.is_finite()) {
            return Err(Error("contact normal exceeds finite range"));
        }
    }
    Err(Error(
        "contact normal alignment did not converge within diagnostic bound",
    ))
}

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub start: [f32; 4],
    pub end: [f32; 4],
    pub low: f32,
    pub high: f32,
}

/// Cast along the body's Y basis from denominator+.25 to lower_stop-.25.
/// Local offset W is forced to zero and the offset anchor W to one, before
/// endpoint multiplication. Radius and offset provenance belong to the caller.
pub fn prepare(
    basis: [[f32; 4]; 4],
    anchor: [[f32; 4]; 4],
    offset: [[f32; 4]; 4],
    radius: [f32; 4],
    lower_stop: [f32; 4],
    denominator: [f32; 4],
) -> Result<[Ray; 4], Error> {
    if basis
        .iter()
        .flatten()
        .chain(anchor.iter().flatten())
        .chain(offset.iter().flatten())
        .chain(&radius)
        .chain(&lower_stop)
        .chain(&denominator)
        .any(|v| !v.is_finite())
        || radius.iter().any(|v| *v <= 0.)
        || denominator.iter().any(|v| *v <= 0.)
    {
        return Err(Error("invalid wheel cast inputs"));
    }
    let rays: [Ray; 4] = std::array::from_fn(|w| {
        let mut center: [f32; 4] = std::array::from_fn(|i| {
            let translated = (0. * basis[3][i] + offset[w][1] * basis[1][i])
                + (offset[w][2] * basis[2][i] + offset[w][0] * basis[0][i]);
            (translated + anchor[w][i]) - radius[w] * basis[1][i]
        });
        center[3] = 1.;
        let low = lower_stop[w] - 0.25;
        let high = denominator[w] + 0.25;
        Ray {
            low,
            high,
            start: std::array::from_fn(|i| high * basis[1][i] + center[i]),
            end: std::array::from_fn(|i| low * basis[1][i] + center[i]),
        }
    });
    if rays.iter().any(|r| {
        r.start
            .iter()
            .chain(&r.end)
            .chain([&r.low, &r.high])
            .any(|v| !v.is_finite())
    }) {
        return Err(Error("wheel cast exceeds finite range"));
    }
    Ok(rays)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_wheel_cast_spans_travel_and_margin() {
        let basis = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [10., 20., 30., 1.],
        ];
        let rays = prepare(
            basis,
            [[1., 2., 3., 7.]; 4],
            [[0., 0., 0., 99.]; 4],
            [0.5; 4],
            [-0.1; 4],
            [0.2; 4],
        )
        .unwrap();
        assert_eq!(rays[0].start, [1., 1.95, 3., 1.]);
        assert_eq!(rays[0].end, [1., 1.15, 3., 1.]);
    }
    #[test]
    fn normal_adjustment_converges_toward_axis_and_rejects_singular_inputs() {
        let adjusted = regularize([0., 1., 0., 0.], [1., 0., 0., 0.]).unwrap();
        assert!(adjusted[0] >= f32::from_bits(0x3e31_26e9));
        assert!((adjusted.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 1e-6);
        assert!(regularize([0.; 4], [0.; 4]).is_err());
        assert!(regularize([0., 1., 0., 0.], [0., -1., 0., 0.]).is_err());
    }
}
