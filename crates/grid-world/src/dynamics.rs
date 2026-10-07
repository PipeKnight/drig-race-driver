//! Conditional scheduling and application of caller-supplied solver results.
//! Wheel/contact equations, external pose integration and frame timing are separate.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Substeps {
    pub count: u32,
    pub delta: f32,
    pub weight_decrement: f32,
}

/// Observed embedded wheel/dynamics loop. The quarter-second bound is our
/// diagnostic limit; it is not an observed original frame-delta clamp.
pub fn substeps(delta: f32) -> Result<Substeps, Error> {
    if !delta.is_finite() || delta <= 0. || delta > 0.25 {
        return Err(Error(
            "solver delta must be positive and at most 0.25 seconds",
        ));
    }
    // Stored reference factor is one f32 step below 1000, not exactly 1000.
    let count = ((delta * f32::from_bits(0x4479_ffff)) as u32).max(1);
    let weight_decrement = 1. / count as f32;
    Ok(Substeps {
        count,
        delta: weight_decrement * delta,
        weight_decrement,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct BodyState {
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub position: [f32; 4],
}

/// Mass, principal inertia components and the observed four-row body basis.
/// The caller must supply the basis used by the contact solver; its synchronization
/// and any temporary drivetrain inertia overrides are separate contracts.
#[derive(Clone, Copy, Debug)]
pub struct ImpulseBody {
    pub mass: f32,
    pub inertia: [f32; 3],
    pub basis: [[f32; 4]; 4],
}

/// Angular velocity change from a supplied world impulse and lever arm.
/// Uses the observed f32 grouping; generates no wheel or contact impulses.
pub fn angular_impulse(
    body: ImpulseBody,
    impulse: [f32; 4],
    lever: [f32; 4],
) -> Result<[f32; 4], Error> {
    if !body.mass.is_finite()
        || body.mass <= 0.
        || body.inertia.iter().any(|v| !v.is_finite() || *v <= 0.)
        || body
            .basis
            .into_iter()
            .flatten()
            .chain(impulse)
            .chain(lever)
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid impulse body or vectors"));
    }
    let torque = [
        lever[1] * impulse[2] - lever[2] * impulse[1],
        lever[2] * impulse[0] - lever[0] * impulse[2],
        lever[0] * impulse[1] - lever[1] * impulse[0],
        0.,
    ];
    let local: [f32; 4] = std::array::from_fn(|i| {
        if i == 3 {
            return 0.;
        }
        let row = body.basis[i];
        let first = torque[0] * row[0] + torque[1] * row[1];
        let second = torque[2] * row[2] + torque[3] * row[3];
        (second + first) / body.inertia[i]
    });
    let result = std::array::from_fn(|i| {
        let first = local[3] * body.basis[3][i] + local[1] * body.basis[1][i];
        let second = local[2] * body.basis[2][i] + local[0] * body.basis[0][i];
        first + second
    });
    if result.iter().any(|v: &f32| !v.is_finite()) {
        return Err(Error("angular impulse exceeds finite range"));
    }
    Ok(result)
}

/// Apply a caller-supplied impulse to velocity only. No position/rotation advance.
pub fn apply_impulse(
    body: ImpulseBody,
    linear: [f32; 4],
    angular: [f32; 4],
    impulse: [f32; 4],
    lever: [f32; 4],
) -> Result<([f32; 4], [f32; 4]), Error> {
    let change = angular_impulse(body, impulse, lever)?;
    if linear.into_iter().chain(angular).any(|v| !v.is_finite()) {
        return Err(Error("nonfinite impulse velocity"));
    }
    let inverse_mass = 1. / body.mass;
    let new_linear = std::array::from_fn(|i| inverse_mass * impulse[i] + linear[i]);
    let new_angular = std::array::from_fn(|i| angular[i] + change[i]);
    if new_linear
        .into_iter()
        .chain(new_angular)
        .any(|v| !v.is_finite())
    {
        return Err(Error("impulse velocity exceeds finite range"));
    }
    Ok((new_linear, new_angular))
}

/// Velocity at a supplied world point: linear plus angular cross its lever arm.
/// This reads the supplied state without advancing it.
pub fn point_velocity(state: BodyState, point: [f32; 4]) -> Result<[f32; 4], Error> {
    if state
        .linear
        .into_iter()
        .chain(state.angular)
        .chain(state.position)
        .chain(point)
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite point velocity input"));
    }
    let lever: [f32; 4] = std::array::from_fn(|i| point[i] - state.position[i]);
    let rotation = [
        state.angular[1] * lever[2] - state.angular[2] * lever[1],
        state.angular[2] * lever[0] - state.angular[0] * lever[2],
        state.angular[0] * lever[1] - state.angular[1] * lever[0],
        0.,
    ];
    let velocity = std::array::from_fn(|i| state.linear[i] + rotation[i]);
    if lever.into_iter().chain(velocity).any(|v| !v.is_finite()) {
        return Err(Error("point velocity exceeds finite range"));
    }
    Ok(velocity)
}

/// Apply a supplied world impulse at a world point, retaining the original pose.
pub fn apply_impulse_at_point(
    body: ImpulseBody,
    state: BodyState,
    impulse: [f32; 4],
    point: [f32; 4],
) -> Result<BodyState, Error> {
    if state
        .position
        .into_iter()
        .chain(point)
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite world impulse point"));
    }
    let lever = std::array::from_fn(|i| point[i] - state.position[i]);
    let (linear, angular) = apply_impulse(body, state.linear, state.angular, impulse, lever)?;
    Ok(BodyState {
        linear,
        angular,
        ..state
    })
}

/// Apply only the observed post-solver additions. This does not advance position
/// by current velocity or integrate rotation; the linked solver handles those
/// separately. Four lanes are retained for exact diagnostic comparisons.
pub fn apply_solver_contribution(
    state: BodyState,
    delta: f32,
    linear_rate: [f32; 4],
    angular_rate: [f32; 4],
) -> Result<BodyState, Error> {
    substeps(delta)?;
    if state
        .linear
        .into_iter()
        .chain(state.angular)
        .chain(state.position)
        .chain(linear_rate)
        .chain(angular_rate)
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite solver contribution"));
    }
    let half_delta = delta * 0.5;
    let result = BodyState {
        linear: std::array::from_fn(|i| delta * linear_rate[i] + state.linear[i]),
        angular: std::array::from_fn(|i| delta * angular_rate[i] + state.angular[i]),
        position: std::array::from_fn(|i| {
            (delta * linear_rate[i]) * half_delta + state.position[i]
        }),
    };
    if result
        .linear
        .into_iter()
        .chain(result.angular)
        .chain(result.position)
        .any(|v| !v.is_finite())
    {
        return Err(Error("solver contribution exceeds finite range"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_body() -> ImpulseBody {
        ImpulseBody {
            mass: 2.,
            inertia: [3., 4., 5.],
            basis: [
                [1., 0., 0., 0.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [0., 0., 0., 1.],
            ],
        }
    }

    #[test]
    fn impulse_at_center_changes_only_linear_velocity() {
        let (linear, angular) = apply_impulse(
            identity_body(),
            [0.; 4],
            [0.; 4],
            [2., -4., 6., 0.],
            [0.; 4],
        )
        .unwrap();
        assert_eq!(linear, [1., -2., 3., 0.]);
        assert_eq!(angular, [0.; 4]);
    }

    #[test]
    fn translated_point_velocity_and_impulse_preserve_pose() {
        let state = BodyState {
            linear: [1., 2., 3., 0.],
            angular: [0., 2., 0., 0.],
            position: [10., -20., 30., 1.],
        };
        let point = [13., -20., 30., 1.];
        assert_eq!(point_velocity(state, point).unwrap(), [1., 2., -3., 0.]);
        let result =
            apply_impulse_at_point(identity_body(), state, [0., 0., 4., 0.], point).unwrap();
        assert_eq!(result.position, state.position);
        assert_eq!(result.linear, [1., 2., 5., 0.]);
        assert_eq!(result.angular, [0., -1., 0., 0.]);
        assert!(point_velocity(state, [f32::NAN; 4]).is_err());
        assert!(
            apply_impulse_at_point(identity_body(), state, [0.; 4], [f32::INFINITY; 4]).is_err()
        );
    }

    #[test]
    fn off_center_impulse_uses_rotated_principal_inertia() {
        let mut body = identity_body();
        // Local X points along world Z, local Z along negative world X.
        body.basis = [
            [0., 0., 1., 0.],
            [0., 1., 0., 0.],
            [-1., 0., 0., 0.],
            [0., 0., 0., 1.],
        ];
        let response = angular_impulse(body, [0., 3., 0., 0.], [1., 0., 0., 0.]).unwrap();
        assert_eq!(response, [0., 0., 1., 0.]);
        body.mass = 0.;
        assert!(angular_impulse(body, [0.; 4], [0.; 4]).is_err());
        body.mass = 2.;
        body.inertia[0] = -1.;
        assert!(angular_impulse(body, [0.; 4], [0.; 4]).is_err());
        body.inertia[0] = 3.;
        body.basis[0][0] = f32::NAN;
        assert!(angular_impulse(body, [0.; 4], [0.; 4]).is_err());
    }

    #[test]
    fn reject_unbounded_or_nonfinite_steps_and_results() {
        for delta in [0., -0.01, 0.251, f32::NAN, f32::INFINITY] {
            assert!(substeps(delta).is_err());
        }
        let zero = BodyState {
            linear: [0.; 4],
            angular: [0.; 4],
            position: [0.; 4],
        };
        assert!(apply_solver_contribution(zero, 0.01, [f32::NAN; 4], [0.; 4]).is_err());
        let overflow = BodyState {
            linear: [f32::MAX; 4],
            ..zero
        };
        assert!(apply_solver_contribution(overflow, 0.25, [f32::MAX; 4], [0.; 4]).is_err());
    }
}
