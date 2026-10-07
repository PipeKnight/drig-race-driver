//! Conditional four-wheel response preparation with the extra tyre axes inactive.
use crate::{
    Error,
    dynamics::{self, BodyState, ImpulseBody},
};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub point: [f32; 4],
    pub axis: [f32; 4],
    pub projection_axis: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Coupling {
    pub speeds: [f32; 4],
    /// Rows are impulse-generating wheels; columns are receiving wheels.
    pub response: [[f32; 4]; 4],
    pub secondary: [[f32; 4]; 4],
}

fn dot(axis: [f32; 4], velocity: [f32; 4]) -> f32 {
    (axis[3] * velocity[3] + axis[2] * velocity[2])
        + (axis[1] * velocity[1] + axis[0] * velocity[0])
}

/// Reproduce the observed test-impulse preparation for four supplied wheel points.
/// Does not compute points/axes, predict pose, mutate inertia or generate forces.
pub fn prepare(body: ImpulseBody, state: BodyState, wheels: [Wheel; 4]) -> Result<Coupling, Error> {
    let strength = body.mass * f32::from_bits(0x38d1_b717);
    let inverse_strength = 1. / strength;
    if strength <= 0.
        || !strength.is_finite()
        || !inverse_strength.is_finite()
        || wheels
            .into_iter()
            .flat_map(|w| w.point.into_iter().chain(w.axis).chain(w.projection_axis))
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid wheel coupling inputs"));
    }
    let mut velocities = [[0.; 4]; 4];
    let mut result = Coupling {
        speeds: [0.; 4],
        response: [[0.; 4]; 4],
        secondary: [[0.; 4]; 4],
    };
    for (i, wheel) in wheels.iter().enumerate() {
        velocities[i] = dynamics::point_velocity(state, wheel.point)?;
        result.speeds[i] = dot(wheel.axis, velocities[i]);
    }
    for (initiator, wheel) in wheels.iter().enumerate() {
        let impulse = std::array::from_fn(|i| strength * wheel.axis[i]);
        let changed = dynamics::apply_impulse_at_point(body, state, impulse, wheel.point)?;
        for (receiver, other) in wheels.iter().enumerate() {
            let velocity = dynamics::point_velocity(changed, other.point)?;
            result.response[initiator][receiver] =
                (dot(other.axis, velocity) - result.speeds[receiver]) * inverse_strength;
            let difference: [f32; 4] =
                std::array::from_fn(|i| inverse_strength * (velocity[i] - velocities[receiver][i]));
            let parallel = dot(other.axis, difference);
            let remainder = std::array::from_fn(|i| difference[i] - parallel * other.axis[i]);
            let denominator = dot(wheel.projection_axis, other.axis);
            if denominator == 0. || !denominator.is_finite() {
                return Err(Error("singular wheel coupling projection"));
            }
            let seed = crate::spawn::reference_seed(denominator, true)?;
            let scaled = seed * dot(wheel.projection_axis, remainder);
            result.secondary[initiator][receiver] = (1. - denominator * seed) * scaled + scaled;
        }
    }
    if result
        .speeds
        .into_iter()
        .chain(result.response.into_iter().flatten())
        .chain(result.secondary.into_iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error("wheel coupling exceeds finite range"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_points_have_inverse_mass_response_without_rotation() {
        let body = ImpulseBody {
            mass: 2.,
            inertia: [3., 4., 5.],
            basis: [
                [1., 0., 0., 0.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [10., 20., 30., 1.],
            ],
        };
        let state = BodyState {
            linear: [0.; 4],
            angular: [0.; 4],
            position: body.basis[3],
        };
        let wheel = Wheel {
            point: state.position,
            axis: [0., 1., 0., 0.],
            projection_axis: [0., 1., 0., 0.],
        };
        let result = prepare(body, state, [wheel; 4]).unwrap();
        assert_eq!(result.speeds, [0.; 4]);
        assert_eq!(result.response, [[0.5; 4]; 4]);
        assert_eq!(result.secondary, [[0.; 4]; 4]);
        let invalid = Wheel {
            projection_axis: [1., 0., 0., 0.],
            ..wheel
        };
        assert!(prepare(body, state, [invalid; 4]).is_err());
    }
}
