//! Supplied-state pose prediction. No contacts, forces or frame scheduling.
use crate::{Error, dynamics::BodyState, spawn::reference_seed};

fn sin_cos(angle: f32) -> (f32, f32) {
    let scaled = f32::from_bits(0x3f22_f983) * angle;
    let half = f32::from_bits(0x3f00_0000 | (scaled.to_bits() & 0x8000_0000));
    let quadrant = (half + scaled) as i32;
    let q = quadrant as f32;
    let r = (angle - f32::from_bits(0x3fc9_0fda) * q) - f32::from_bits(0x33a2_2169) * q;
    let z = r * r;
    let cosine = ((f32::from_bits(0xbab2_4993) * z + f32::from_bits(0x3d2a_a036)) * z
        + f32::from_bits(0xbeff_ffdf))
        * z
        + 1.;
    let sine = ((f32::from_bits(0xb94c_8c6e) * z + f32::from_bits(0x3c08_8342)) * z
        + f32::from_bits(0xbe2a_aaa1))
        * (r * z)
        + r;
    let sign = |value: f32, negative: bool| {
        f32::from_bits(value.to_bits() ^ if negative { 0x8000_0000 } else { 0 })
    };
    let n = quadrant & 3;
    (
        sign(if n & 1 == 0 { sine } else { cosine }, n & 2 != 0),
        sign(
            if (n + 1) & 1 == 0 { sine } else { cosine },
            (n + 1) & 2 != 0,
        ),
    )
}

/// Reference polynomial Rodrigues rotation, bounded to ±64 radians for diagnostics.
/// The supplied axis is not normalized here; its fourth lane is masked away.
pub fn rotation(axis: [f32; 4], angle: f32) -> Result<[[f32; 4]; 4], Error> {
    if !angle.is_finite() || angle.abs() > 64. || axis.iter().any(|v| !v.is_finite()) {
        return Err(Error("invalid bounded rotation input"));
    }
    let (s, c) = sin_cos(angle);
    let t = 1. - c;
    let [x, y, z, _] = axis;
    let negative = |v: f32| f32::from_bits(v.to_bits() ^ 0x8000_0000);
    let cross = [
        [c, s * z, negative(s * y), 0.],
        [negative(s * z), c, s * x, 0.],
        [s * y, negative(s * x), c, 0.],
    ];
    let result = std::array::from_fn(|row| {
        if row == 3 {
            return [0., 0., 0., 1.];
        }
        std::array::from_fn(|col| {
            let a = if col == 3 { 0. } else { axis[col] };
            (axis[row] * a) * t + cross[row][col]
        })
    });
    if result.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("rotation exceeds finite range"));
    }
    Ok(result)
}

/// Advance position and basis using supplied linear/angular velocity. The basis's
/// fourth row is the position, and is restored after rotation multiplication.
pub fn predict(
    mut basis: [[f32; 4]; 4],
    state: BodyState,
    delta: f32,
) -> Result<[[f32; 4]; 4], Error> {
    if !delta.is_finite()
        || !(0. ..=0.25).contains(&delta)
        || basis
            .iter()
            .flatten()
            .chain(state.linear.iter())
            .chain(state.angular.iter())
            .chain(state.position.iter())
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid pose prediction input"));
    }
    let position = std::array::from_fn(|i| delta * state.linear[i] + state.position[i]);
    let w = state.angular;
    let squared = (w[3] * w[3] + w[2] * w[2]) + (w[1] * w[1] + w[0] * w[0]);
    // RSQRTPS treats subnormal inputs as zero; refinement becomes NaN and the
    // original unordered comparison skips rotation. Preserve that branch.
    let length = if squared < f32::MIN_POSITIVE {
        0.
    } else {
        let seed = reference_seed(squared, false)?;
        let root = seed * squared;
        (1. - root * seed) * (root * 0.5) + root
    };
    if !length.is_finite() {
        return Err(Error("angular magnitude exceeds finite range"));
    }
    if length > f32::from_bits(0x2338_77aa) {
        let inverse = 1. / length;
        let rotation = rotation(w.map(|v| inverse * v), length * delta)?;
        for row in &mut basis[..3] {
            let old = *row;
            *row = std::array::from_fn(|i| {
                (old[2] * rotation[2][i] + old[0] * rotation[0][i])
                    + (old[1] * rotation[1][i] + old[3] * rotation[3][i])
            });
        }
    }
    basis[3] = position;
    if basis.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("predicted pose exceeds finite range"));
    }
    Ok(basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_and_quarter_turn_follow_row_vector_convention() {
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [10., 20., 30., 1.],
        ];
        let state = BodyState {
            linear: [2., -4., 6., 0.],
            angular: [0.; 4],
            position: identity[3],
        };
        let moved = predict(identity, state, 0.25).unwrap();
        assert_eq!(moved[3], [10.5, 19., 31.5, 1.]);
        assert_eq!(moved[..3], identity[..3]);
        let turn = rotation([0., 1., 0., 0.], std::f32::consts::FRAC_PI_2).unwrap();
        assert!((turn[0][2] + 1.).abs() < 0.000001);
        assert!((turn[2][0] - 1.).abs() < 0.000001);
        assert!(rotation([0.; 4], 65.).is_err());
        assert!(predict(identity, state, -0.01).is_err());
    }
}
