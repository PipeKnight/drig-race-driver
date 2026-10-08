//! Four-wheel steering pointer selection and spans from supplied coordinates.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub scale_e54: f32,
    pub pair: [usize; 2],
    pub span_21d4: f32,
    pub span_21d8: f32,
}
// Retain the original multiply-then-add operand order at each accumulation.
#[allow(clippy::assign_op_pattern)]
pub fn initialize(points: [[f32; 4]; 4], property_340: f32) -> Result<Geometry, Error> {
    if !property_340.is_finite() || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("nonfinite steering geometry input"));
    }
    let mut cut_x = 0_f32;
    let mut cut_z = 0_f32;
    for p in points {
        cut_x = p[0] * 0.25 + cut_x;
        cut_z = p[2] * 0.25 + cut_z;
    }
    let mut selected_z = 0_f32;
    let mut other_z = 0_f32;
    let mut pair = [None; 2];
    for (i, p) in points.iter().enumerate() {
        if p[2] > cut_z {
            selected_z = p[2] * 0.5 + selected_z;
            pair[usize::from(p[0] > cut_x)] = Some(i);
        } else {
            other_z = p[2] * 0.5 + other_z;
        }
        // The original updates both cuts again during the selection pass.
        cut_x = p[0] * 0.25 + cut_x;
        cut_z = p[2] * 0.25 + cut_z;
    }
    let [Some(first), Some(second)] = pair else {
        return Err(Error("steering geometry lacks paired wheels"));
    };
    let span_21d4 = points[second][0] - points[first][0];
    let span_21d8 = selected_z - other_z;
    if !span_21d4.is_finite() || !span_21d8.is_finite() {
        return Err(Error("nonfinite steering geometry output"));
    }
    Ok(Geometry {
        scale_e54: property_340,
        pair: [first, second],
        span_21d4,
        span_21d8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_pair_is_rejected() {
        assert!(initialize([[0.; 4]; 4], 0.5).is_err());
        assert!(initialize([[0.; 4]; 4], f32::INFINITY).is_err());
    }
}
