//! Four-wheel production surface slowdown with supplied contacts and body state.
use crate::{
    Error,
    dynamics::{self, ImpulseBody},
    spawn::reference_seed,
};

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub minimum_speed: f32,
    pub maximum_speed: f32,
    pub slowdown: f32,
    /// Original +73; source loader initializes this false, later meaning open.
    pub flag_73: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    /// Supplied relative lever from the body's pointed view, not a world point.
    pub lever: [f32; 4],
    pub spin: f32,
    pub radius: f32,
    pub additive_22b4: f32,
    pub material: Option<Material>,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub body: ImpulseBody,
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub wheels: [Wheel; 4],
    pub speed: f32,
    pub delta: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub spins: [f32; 4],
}
pub fn apply(input: Input) -> Result<Output, Error> {
    if !input.delta.is_finite()
        || input.delta < 0.
        || input.delta > 0.1
        || !input.speed.is_finite()
        || !input.body.mass.is_finite()
        || input.body.mass <= 0.
        || input
            .body
            .inertia
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        || input
            .body
            .basis
            .into_iter()
            .flatten()
            .chain(input.linear)
            .chain(input.angular)
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid supplied surface drag body"));
    }
    let mut coefficients = [0.; 4];
    let all_flagged = input
        .wheels
        .iter()
        .all(|w| w.material.is_some_and(|m| m.flag_73));
    for (i, w) in input.wheels.iter().enumerate() {
        if w.lever
            .iter()
            .chain([w.spin, w.radius, w.additive_22b4].iter())
            .any(|v| !v.is_finite())
            || w.radius <= 0.
        {
            return Err(Error("invalid supplied surface drag wheel"));
        }
        if let Some(m) = w.material {
            if [m.minimum_speed, m.maximum_speed, m.slowdown]
                .iter()
                .any(|v| !v.is_finite())
                || m.maximum_speed == m.minimum_speed
            {
                return Err(Error("invalid supplied surface drag material"));
            }
            let mut scale = ((input.speed - m.minimum_speed) / (m.maximum_speed - m.minimum_speed))
                .clamp(0., 1.);
            if m.flag_73 && all_flagged {
                scale = ((input.speed - 3.) * f32::from_bits(0x3eaa_aaab)).clamp(0., 1.);
            }
            coefficients[i] = m.slowdown * scale + w.additive_22b4;
        }
    }
    let average = (((coefficients[0] + coefficients[1]) + coefficients[2]) + coefficients[3])
        * 0.25
        * f32::from_bits(0x3f73_3333);
    let coefficients = coefficients.map(|v| v * f32::from_bits(0x3d4c_ccd0) + average);
    if coefficients.iter().any(|v| !v.is_finite()) {
        return Err(Error("surface drag coefficient exceeds finite range"));
    }
    let mut linear = input.linear;
    let mut angular = input.angular;
    let mut spins = input.wheels.map(|w| w.spin);
    // Each impulse changes the state used by the next wheel.
    for (i, (w, coefficient)) in input.wheels.iter().zip(coefficients).enumerate() {
        if coefficient <= 0. {
            continue;
        }
        let rotation = [
            angular[1] * w.lever[2] - angular[2] * w.lever[1],
            angular[2] * w.lever[0] - angular[0] * w.lever[2],
            angular[0] * w.lever[1] - angular[1] * w.lever[0],
            0.,
        ];
        let velocity: [f32; 4] = std::array::from_fn(|i| linear[i] + rotation[i]);
        let length = (velocity[0] * velocity[0] + velocity[1] * velocity[1])
            + (velocity[2] * velocity[2] + velocity[3] * velocity[3]);
        if !length.is_finite() || length <= 0. {
            return Err(Error("surface drag requires nonzero finite point velocity"));
        }
        let seed = reference_seed(length, false)?;
        let inverse = (1. - (length * seed) * seed) * (seed * 0.5) + seed;
        let magnitude = -0. - ((input.body.mass * coefficient) * input.delta);
        let impulse = velocity.map(|v| (inverse * v) * magnitude);
        if spins[i] > 0. {
            spins[i] = (spins[i] - (coefficient / w.radius) * input.delta).max(0.);
        }
        (linear, angular) = dynamics::apply_impulse(input.body, linear, angular, impulse, w.lever)?;
    }
    Ok(Output {
        linear,
        angular,
        spins,
    })
}
