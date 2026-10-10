//! Joined four-wheel coupling, suspension substeps and two-pass body finalization.
//! Geometry, contact speeds, rate inputs and alternate inertia are supplied.
//! Extra tyre axes, drivetrain and production contact preparation are absent.
use crate::{Error, dynamics, finalize, wheel_coupling, wheel_step};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub step: wheel_step::Wheel,
    pub point: [f32; 4],
    pub projection_axis: [f32; 4],
    pub clip_upper: f32,
    pub force: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub body: dynamics::ImpulseBody,
    pub state: dynamics::BodyState,
    pub wheels: [Wheel; 4],
    pub delta: f32,
    pub scale: f32,
    pub alternate_inertia: [f32; 3],
    pub linear_rate: [f32; 4],
    pub angular_rate: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub coupling: wheel_coupling::Coupling,
    pub schedule: dynamics::Substeps,
    pub last_weight: f32,
    /// Accumulators and wheel states immediately before finalization.
    pub accumulated: [wheel_step::Wheel; 4],
    pub finalized: finalize::Finalized,
}

/// Couple the supplied geometry to current body state, run the observed
/// delta-dependent wheel schedule, commit weighted pose and raw velocities,
/// then apply the supplied pre-solver velocity rates. No geometry is invented.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    let schedule = dynamics::substeps(input.delta)?;
    if input.body.basis[3].map(f32::to_bits) != input.state.position.map(f32::to_bits) {
        return Err(Error("suspension frame position and basis disagree"));
    }
    if input
        .wheels
        .iter()
        .any(|w| w.clip_upper.to_bits() != w.step.suspension.travel_denominator.to_bits())
    {
        return Err(Error("shared wheel travel and finalizer clip disagree"));
    }
    let coupling = wheel_coupling::prepare(
        input.body,
        input.state,
        input.wheels.map(|w| wheel_coupling::Wheel {
            point: w.point,
            axis: w.step.axis,
            projection_axis: w.projection_axis,
        }),
    )?;
    let mut accumulated = input.wheels.map(|w| w.step);
    for (i, wheel) in accumulated.iter_mut().enumerate() {
        wheel.body_speed = coupling.speeds[i];
    }
    let mut weight = 1.;
    let mut last_weight = weight;
    for _ in 0..schedule.count {
        last_weight = weight;
        accumulated = wheel_step::step(
            accumulated,
            coupling.response,
            coupling.secondary,
            input.scale,
            schedule.delta,
            weight,
        )?;
        weight -= schedule.weight_decrement;
    }
    let wheels = std::array::from_fn(|i| {
        let w = accumulated[i];
        finalize::Wheel {
            point: input.wheels[i].point,
            axis: w.axis,
            projection_axis: input.wheels[i].projection_axis,
            raw: w.raw,
            weighted: w.weighted,
            contact_raw: w.contact_raw,
            contact_weighted: w.contact_weighted,
            displacement: w.suspension.displacement,
            speed: w.suspension.speed,
            floor: w.floor,
            lower: w.lower,
            clip_upper: input.wheels[i].clip_upper,
            force: input.wheels[i].force,
        }
    });
    // The original wrapper adds the whole incoming delta to the reset clock
    // once, rather than summing rounded sub-deltas for finalization.
    let mut finalized = finalize::finish(
        input.body,
        input.state,
        wheels,
        input.alternate_inertia,
        input.delta,
    )?;
    finalized.state = dynamics::apply_solver_contribution(
        finalized.state,
        input.delta,
        input.linear_rate,
        input.angular_rate,
    )?;
    finalized.body.basis[3] = finalized.state.position;
    Ok(Output {
        coupling,
        schedule,
        last_weight,
        accumulated,
        finalized,
    })
}

/// Carry consumed wheel and body state into another supplied-geometry frame.
/// The caller must refresh points, axes, floor and external rates as appropriate;
/// retaining them is useful for controlled sequences, not road simulation.
pub fn carry(input: &Input, output: &Output) -> Input {
    let mut next = *input;
    next.body = output.finalized.body;
    next.state = output.finalized.state;
    for (i, wheel) in next.wheels.iter_mut().enumerate() {
        wheel.step = output.accumulated[i];
        [
            wheel.step.raw,
            wheel.step.weighted,
            wheel.step.contact_raw,
            wheel.step.contact_weighted,
        ] = output.finalized.wheels[i].remaining;
        wheel.force = output.finalized.wheels[i].force;
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carried_frames_consume_impulses_and_preserve_committed_pose() {
        let basis = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let wheel = Wheel {
            step: wheel_step::Wheel {
                suspension: crate::suspension::Wheel {
                    displacement: 0.,
                    speed: 0.,
                    spring: 0.,
                    preload: 0.,
                    travel_denominator: 1.,
                    damper: 0.,
                    speed_threshold: 1.,
                    high_speed_ratio: 1.,
                    negative_speed_scale: 1.,
                    bump: 0.,
                    axle_coupling: 0.,
                },
                // Axis perpendicular to gravity isolates the state handoff.
                axis: [1., 0., 0., 0.],
                body_speed: 0.,
                body_rate: 0.,
                floor: -1.,
                contact_speed: 0.,
                raw: 0.,
                weighted: 0.,
                contact_raw: 0.,
                contact_weighted: 0.,
                contact: false,
                lower: -2.,
                upper: 2.,
            },
            point: basis[3],
            projection_axis: [1., 0., 0., 0.],
            clip_upper: 1.,
            force: 0.,
        };
        let input = Input {
            body: dynamics::ImpulseBody {
                mass: 2.,
                inertia: [3., 4., 5.],
                basis,
            },
            state: dynamics::BodyState {
                linear: [0., 0., 3., 0.],
                angular: [0.; 4],
                position: basis[3],
            },
            wheels: [wheel; 4],
            delta: 0.01,
            scale: 1.,
            alternate_inertia: [6., 8., 10.],
            linear_rate: [0.; 4],
            angular_rate: [0.; 4],
        };
        let first = calculate(&input).unwrap();
        assert_eq!(first.schedule.count, 9);
        assert_eq!(first.finalized.state.position[2], 0.01 * 3.);
        assert_eq!(first.finalized.state.linear, input.state.linear);
        let next = carry(&input, &first);
        assert_eq!(next.body.inertia, input.alternate_inertia);
        assert_eq!(next.body.basis[3], next.state.position);
        assert!(next.wheels.iter().all(|w| [
            w.step.raw,
            w.step.weighted,
            w.step.contact_raw,
            w.step.contact_weighted
        ] == [0.; 4]));
        let second = calculate(&next).unwrap();
        assert_eq!(
            second.finalized.state.position[2],
            (0.01 * 3.) + (0.01 * 3.)
        );
        assert_eq!(second.finalized.state.linear, input.state.linear);
        let mut invalid = input;
        invalid.wheels[0].clip_upper = 0.5;
        assert!(calculate(&invalid).is_err());
    }
}
