//! Visual-deformation wheel level update with component notifications inactive.
use crate::{Error, spawn::reference_seed};

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub previous_level: f32,
    pub angular_deformation_1790: f32,
    pub displacement_1650: [f32; 4],
    pub correction_enabled_c7ae: bool,
    pub correction_scale_eefc: f32,
    pub correction_vector_c740: [f32; 4],
    pub angular_limit_280: f32,
    pub displacement_limit_284: f32,
}

/// Updates one level at mutable18..24. The original does not read its delta.
/// The caller supplies deformation; impact production and notifications are separate.
pub fn calculate(input: Input) -> Result<f32, Error> {
    if input
        .displacement_1650
        .into_iter()
        .chain(input.correction_vector_c740)
        .chain([
            input.previous_level,
            input.angular_deformation_1790,
            input.correction_scale_eefc,
            input.angular_limit_280,
            input.displacement_limit_284,
        ])
        .any(|v| !v.is_finite())
        || input.angular_limit_280 < 0.
        || (input.angular_limit_280 != 0. && input.displacement_limit_284 <= 0.)
    {
        return Err(Error("invalid visual wheel damage input"));
    }
    // Either signed zero disables the entire update, preserving the old word.
    if input.angular_limit_280 == 0. {
        return Ok(input.previous_level);
    }
    let angular = input.angular_deformation_1790.abs()
        / (input.angular_limit_280 * f32::from_bits(0x3c8efa35));
    let mut v = input.displacement_1650;
    if input.correction_enabled_c7ae {
        for (value, correction) in v.iter_mut().zip(input.correction_vector_c740) {
            *value -= input.correction_scale_eefc * correction;
        }
    }
    // Four lanes, with the original SSE reduction and root-refinement order.
    let squared = (v[1] * v[1] + v[0] * v[0]) + (v[3] * v[3] + v[2] * v[2]);
    if !squared.is_finite() || !angular.is_finite() {
        return Err(Error("visual wheel damage exceeds finite range"));
    }
    let length = if squared == 0. {
        0.
    } else {
        let seed = reference_seed(squared, false)?;
        let root = seed * squared;
        (1. - root * seed) * (root * 0.5) + root
    };
    let displacement = length / input.displacement_limit_284;
    if !displacement.is_finite() {
        return Err(Error("nonfinite visual wheel damage result"));
    }
    // Equality selects displacement, and equality with the prior selects new.
    let level = if angular > displacement {
        angular
    } else {
        displacement
    }
    .clamp(0., 1.);
    Ok(if input.previous_level > level {
        input.previous_level
    } else {
        level
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deformation_is_monotonic_and_zero_setting_preserves_signed_word() {
        let mut input = Input {
            previous_level: 0.75,
            angular_deformation_1790: 0.,
            displacement_1650: [0.; 4],
            correction_enabled_c7ae: false,
            correction_scale_eefc: 0.,
            correction_vector_c740: [0.; 4],
            angular_limit_280: 27.,
            displacement_limit_284: 0.45,
        };
        assert_eq!(calculate(input).unwrap(), 0.75);
        input.displacement_1650[3] = 1.;
        assert_eq!(calculate(input).unwrap(), 1.);
        input.angular_limit_280 = -0.;
        input.previous_level = -0.;
        assert_eq!(calculate(input).unwrap().to_bits(), (-0.0f32).to_bits());
        input.angular_limit_280 = 27.;
        input.displacement_limit_284 = 0.;
        assert!(calculate(input).is_err());
    }
}
