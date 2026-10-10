//! Optional physical correction bias for an already constructed contact row.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    pub vectors: [[f32; 4]; 4],
    pub lower: f32,
    pub upper: f32,
    pub bias: f32,
}

pub struct Input {
    pub two_bodies: bool,
    pub disabled: bool,
    pub accumulated: f32,
    pub threshold: f32,
    pub scale: f32,
    pub upper_limit: f32,
    pub active: bool,
    pub constraint: Constraint,
}

pub struct Output {
    pub constraint: Constraint,
    pub active: bool,
    /// Each invocation consumes one existing 192-byte constraint row.
    pub cursor_advance: usize,
}

pub fn calculate(input: &Input) -> Result<Output, Error> {
    if [
        input.accumulated,
        input.threshold,
        input.scale,
        input.upper_limit,
    ]
    .into_iter()
    .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite contact bias operand"));
    }
    let mut constraint = input.constraint;
    let mut active = input.active;
    if input.disabled {
        let count = if input.two_bodies { 4 } else { 2 };
        constraint.vectors[..count].fill([0.; 4]);
    } else {
        let difference = input.accumulated - input.threshold;
        let bias = difference * input.scale;
        if !bias.is_finite() {
            return Err(Error("nonfinite contact bias result"));
        }
        if bias >= 0. {
            active = true;
            constraint.bias = bias; // Keep a nonnegative signed zero's bits.
        } else {
            constraint.bias = 0.;
        }
        constraint.lower = 0.;
        constraint.upper = input.upper_limit;
    }
    Ok(Output {
        constraint,
        active,
        cursor_advance: 192,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            two_bodies: false,
            disabled: false,
            accumulated: 1.,
            threshold: 0.,
            scale: 1.,
            upper_limit: 100.,
            active: false,
            constraint: Constraint {
                vectors: [[2.; 4]; 4],
                lower: -1.,
                upper: 3.,
                bias: 4.,
            },
        }
    }
    #[test]
    fn disabled_single_preserves_unused_vectors_limits_and_flag() {
        let mut i = input();
        i.disabled = true;
        let o = calculate(&i).unwrap();
        assert_eq!(o.constraint.vectors, [[0.; 4], [0.; 4], [2.; 4], [2.; 4]]);
        assert_eq!(
            [o.constraint.lower, o.constraint.upper, o.constraint.bias],
            [-1., 3., 4.]
        );
        assert!(!o.active);
        assert_eq!(o.cursor_advance, 192);
    }
    #[test]
    fn negative_bias_does_not_clear_existing_activity() {
        let mut i = input();
        i.threshold = 2.;
        i.active = true;
        let o = calculate(&i).unwrap();
        assert!(o.active);
        assert_eq!(o.constraint.bias.to_bits(), 0);
        assert_eq!(o.constraint.vectors, i.constraint.vectors);
    }
    #[test]
    fn negative_zero_activates_and_keeps_sign() {
        let mut i = input();
        i.accumulated = -0.;
        let o = calculate(&i).unwrap();
        assert!(o.active);
        assert_eq!(o.constraint.bias.to_bits(), (-0.0f32).to_bits());
        assert_eq!([o.constraint.lower, o.constraint.upper], [0., 100.]);
    }
    #[test]
    fn threshold_equality_activates() {
        let mut i = input();
        i.threshold = i.accumulated;
        assert!(calculate(&i).unwrap().active);
    }
}
