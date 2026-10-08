//! Downstream supplied wheel-history grip multiplier from original 0098db40.
//! Body scalar meanings and the coordinate-dependent flags remain provisional.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// Wheel +11c/+120/+124, in the same order as wheel_history::Wheel::history.
    pub history: [f32; 3],
    pub scalar_1ffc: f32,
    pub scalar_1f24: f32,
    pub scalar_1fa4: f32,
    pub scalar_1fa8: f32,
    pub wheel_coordinate_z: f32,
    pub flag_b12: bool,
    pub flag_227c: bool,
}

#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed multiplier-first floating-point operations"
)]
pub fn factor(input: Input) -> Result<f32, Error> {
    if input
        .history
        .iter()
        .chain(
            [
                input.scalar_1ffc,
                input.scalar_1f24,
                input.scalar_1fa4,
                input.scalar_1fa8,
                input.wheel_coordinate_z,
            ]
            .iter(),
        )
        .any(|v| !v.is_finite())
    {
        return Err(Error("invalid supplied history grip inputs"));
    }
    // These factors are deliberately not clamped by the original.
    let mut history = 1. - input.history[1] * 0.25;
    if input.flag_b12 {
        history = (1. - (1. - input.history[0]) * f32::from_bits(0x3e19_999a)) * history;
    }
    let dirt = (1. - input.history[2] * 0.5) * history;
    let mut result = (1. - (input.scalar_1ffc - 0.5) * f32::from_bits(0x3dcc_cccd)) * dirt;
    result *= input.scalar_1f24;
    if input.flag_227c {
        result =
            (1. - if input.wheel_coordinate_z > 0. {
                input.scalar_1fa4
            } else {
                input.scalar_1fa8
            }) * result;
    }
    if !result.is_finite() {
        return Err(Error("history grip factor exceeds finite range"));
    }
    Ok(result)
}

/// The caller supplies its already-rounded grip before this downstream factor.
pub fn apply(input: Input, grip: f32) -> Result<f32, Error> {
    if !grip.is_finite() {
        return Err(Error("invalid incoming grip"));
    }
    let result = ((factor(input)? as f64) * grip as f64) as f32;
    if !result.is_finite() {
        return Err(Error("history-adjusted grip exceeds finite range"));
    }
    Ok(result)
}
