//! Supplied input conditioning before the wheel coefficient writer.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub command: f32,
    /// Body+2294: enables the conditional perturbation when positive.
    pub perturbation: f32,
    /// Body+1648; physical units remain unverified.
    pub speed_scalar: f32,
    /// Body+159c; source and physical meaning remain unverified.
    pub phase_scalar: f32,
}

pub fn condition(input: Input) -> Result<f32, Error> {
    if [
        input.command,
        input.perturbation,
        input.speed_scalar,
        input.phase_scalar,
    ]
    .into_iter()
    .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite steering input"));
    }
    let mut command = input.command;
    if input.perturbation > 0. {
        let speed = (input.speed_scalar * 0.2).clamp(0., 1.);
        let gain = (input.perturbation + 1.) * 0.5;
        let first = f64::from(input.phase_scalar * 8.).sin() as f32;
        let second = f64::from(input.phase_scalar * 5.3).sin() as f32;
        let product = first * second;
        let wave = if input.perturbation < 0.5 {
            -0.0 - product.abs()
        } else {
            product
        };
        command += ((wave * gain) * speed) * 0.14;
    }
    let result = command.clamp(-1., 1.);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(Error("nonfinite conditioned steering input"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_clamp_and_signed_zero() {
        for (command, expected) in [(-2., -1_f32), (2., 1.), (-0.0, -0.0), (0., 0.)] {
            let actual = condition(Input {
                command,
                perturbation: 0.,
                speed_scalar: 10.,
                phase_scalar: 3.,
            })
            .unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        assert!(
            condition(Input {
                command: f32::NAN,
                perturbation: 0.,
                speed_scalar: 0.,
                phase_scalar: 0.
            })
            .is_err()
        );
    }
}
