//! Five supplied angle/radius samples; no deformation or damage state generation.
use crate::Error;

/// Nearest angular sample wins; equal distances retain the earlier sample.
/// Angles are radians. Diagnostic bounds keep the supplied-state arithmetic finite.
pub fn adjustment(angle: f32, radius: f32, samples: [[f32; 2]; 5]) -> Result<f32, Error> {
    Ok(adjustment_wide(angle, radius, samples)? as f32)
}

pub(crate) fn adjustment_wide(
    angle: f32,
    radius: f32,
    samples: [[f32; 2]; 5],
) -> Result<f64, Error> {
    if !angle.is_finite()
        || angle.abs() > 1_000_000.
        || !radius.is_finite()
        || radius <= 0.
        || samples
            .iter()
            .any(|[a, r]| !a.is_finite() || a.abs() > 1_000_000. || !r.is_finite() || *r <= 0.)
    {
        return Err(Error("invalid wheel radius samples"));
    }
    let pi = f32::from_bits(0x4049_0fdb);
    let turn = f32::from_bits(0x40c9_0fdb);
    let phase = pi - angle;
    let mut nearest = f32::MAX;
    let mut selected = radius;
    for [sample_angle, sample_radius] in samples {
        let remainder = (f64::from(sample_angle) - f64::from(phase)).abs() % f64::from(turn);
        let distance = if remainder > f64::from(pi) {
            turn - remainder as f32
        } else {
            remainder as f32
        };
        if distance < nearest {
            nearest = distance;
            selected = sample_radius;
        }
    }
    Ok(f64::from(selected) - f64::from(radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angular_wrap_and_first_equal_sample() {
        let pi = f32::from_bits(0x4049_0fdb);
        let turn = f32::from_bits(0x40c9_0fdb);
        let samples = [[turn, 0.4], [0., 0.5], [1., 0.6], [2., 0.7], [3., 0.8]];
        assert_eq!(adjustment(pi, 0.3, samples).unwrap(), 0.4_f32 - 0.3_f32);
        assert!(adjustment(0., 0., samples).is_err());
    }
}
