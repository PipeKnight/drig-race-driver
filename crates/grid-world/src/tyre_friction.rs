//! Active slip limiter and contact impulses for supplied tyre coefficients.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub normal_impulse: f32,
    pub radius: f32,
    pub spin: f32,
    pub inertia: f32,
    pub normal_scale: f32,
    pub lateral_denominator: f32,
    pub lateral: f32,
    pub displacement: f32,
    pub rate: f32,
    pub strength: f32,
    pub slide: f32,
    pub delta: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub displacement: f32,
    pub first_impulse: f32,
    pub second_impulse: f32,
    pub angular_delta: f32,
    pub limited: bool,
}

#[allow(clippy::assign_op_pattern)] // Preserve original factor-first operand order.
pub fn resolve(input: Input) -> Result<Output, Error> {
    let Input {
        normal_impulse,
        radius,
        spin,
        inertia,
        normal_scale,
        lateral_denominator,
        lateral,
        displacement,
        rate,
        strength,
        slide,
        delta,
    } = input;
    if [
        normal_impulse,
        radius,
        spin,
        inertia,
        normal_scale,
        lateral_denominator,
        lateral,
        displacement,
        rate,
        strength,
        slide,
        delta,
    ]
    .iter()
    .any(|v| !v.is_finite())
        || normal_impulse < 0.
        || radius <= 0.
        || inertia <= 0.
        || lateral_denominator <= 0.
        || delta <= 0.
        || delta > 0.01
    {
        return Err(Error("invalid supported tyre friction inputs"));
    }
    let speed_scale = (spin * radius).abs().max(1.);
    let load_scale = (normal_scale * normal_impulse).clamp(f32::from_bits(0x3dcc_cccd), 10.);
    let mut displacement = rate * delta + displacement;
    let mut second = displacement / (radius * f32::from_bits(0x3dcc_cccd));
    let mut first = lateral / ((lateral_denominator * load_scale) * speed_scale);
    let squared = first * first + second * second;
    let limited = squared > 1.;
    if limited {
        let root = squared.sqrt();
        let drop = (root - 1.).clamp(0., 1.);
        let factor = (1. - slide * drop) * (1. / root);
        first = factor * first;
        second = factor * second;
        displacement *= factor;
    }
    let first_impulse = -0. - (strength * first) * normal_impulse;
    let second_impulse = -0. - (strength * second) * normal_impulse;
    let angular_delta = ((-1. / inertia) * radius) * second_impulse;
    if [displacement, first_impulse, second_impulse, angular_delta]
        .iter()
        .any(|v| !v.is_finite())
    {
        return Err(Error("tyre friction exceeds finite range"));
    }
    Ok(Output {
        displacement,
        first_impulse,
        second_impulse,
        angular_delta,
        limited,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            normal_impulse: 10.,
            radius: 1.,
            spin: 0.,
            inertia: 1.,
            normal_scale: 1.,
            lateral_denominator: 1.,
            lateral: 0.,
            displacement: 0.2,
            rate: 0.,
            strength: 1.,
            slide: 0.25,
            delta: 0.001,
        }
    }
    #[test]
    fn saturated_slip_reduces_retained_state_and_applies_rotational_response() {
        let result = resolve(input()).unwrap();
        assert!(result.limited);
        assert_eq!(result.displacement, 0.075);
        assert_eq!(result.second_impulse, -7.5);
        assert_eq!(result.angular_delta, 7.5);
        assert_eq!(result.first_impulse.to_bits(), (-0.0f32).to_bits());
    }
    #[test]
    fn absent_load_keeps_force_zero_and_singular_inertia_is_rejected() {
        let mut input = input();
        input.normal_impulse = 0.;
        let result = resolve(input).unwrap();
        assert_eq!(result.first_impulse, 0.);
        assert_eq!(result.second_impulse, 0.);
        assert_eq!(result.angular_delta, 0.);
        input.inertia = 0.;
        assert!(resolve(input).is_err());
    }
}
