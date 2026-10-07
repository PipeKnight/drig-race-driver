//! Two-pass body finalization for four wheels, extra tyre axes inactive.
//! Contact-object floor adjustments and contact-pointer ownership are separate.
use crate::{
    Error,
    dynamics::{BodyState, ImpulseBody, apply_impulse_at_point},
    pose,
};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub point: [f32; 4],
    pub axis: [f32; 4],
    pub projection_axis: [f32; 4],
    pub raw: f32,
    pub weighted: f32,
    pub contact_raw: f32,
    pub contact_weighted: f32,
    pub displacement: f32,
    pub speed: f32,
    /// Supplied floor with no contact-object adjustment.
    pub floor: f32,
    pub lower: f32,
    /// Wheel +4c clipping value; separate from the substep's +50 stop bound.
    pub clip_upper: f32,
    pub force: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WheelResult {
    pub displacement: f32,
    pub speed: f32,
    pub floor_difference: f32,
    pub force: f32,
    /// Consumed raw/weighted suspension and raw/weighted contact accumulators.
    pub remaining: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Finalized {
    pub body: ImpulseBody,
    pub state: BodyState,
    pub wheels: [WheelResult; 4],
    pub clock: f32,
}

fn pass(
    body: ImpulseBody,
    mut state: BodyState,
    wheels: [Wheel; 4],
    weighted: bool,
) -> Result<BodyState, Error> {
    for wheel in wheels {
        let strength = if weighted { wheel.weighted } else { wheel.raw };
        state = apply_impulse_at_point(body, state, wheel.axis.map(|v| strength * v), wheel.point)?;
        let a = wheel.axis;
        let p = wheel.projection_axis;
        let dot = (p[3] * a[3] + p[2] * a[2]) + (p[1] * a[1] + p[0] * a[0]);
        if dot == 0. || !dot.is_finite() {
            return Err(Error("singular finalizer projection"));
        }
        let strength = if weighted {
            wheel.contact_weighted
        } else {
            wheel.contact_raw
        };
        let scale = strength / dot;
        let impulse = std::array::from_fn(|i| (p[i] - dot * a[i]) * scale);
        state = apply_impulse_at_point(body, state, impulse, wheel.point)?;
    }
    Ok(state)
}

/// Compute committed pose from weighted impulses, restore initial state and
/// compute final velocities from raw impulses. Alternate inertia is retained.
/// The caller commits the returned consumed accumulators and reset clock.
pub fn finish(
    mut body: ImpulseBody,
    state: BodyState,
    wheels: [Wheel; 4],
    alternate_inertia: [f32; 3],
    delta: f32,
) -> Result<Finalized, Error> {
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.25
        || alternate_inertia.iter().any(|v| !v.is_finite() || *v <= 0.)
        || wheels.iter().any(|w| {
            [
                w.raw,
                w.weighted,
                w.contact_raw,
                w.contact_weighted,
                w.displacement,
                w.speed,
                w.floor,
                w.lower,
                w.clip_upper,
                w.force,
            ]
            .iter()
            .any(|v| !v.is_finite())
                || w.lower > w.clip_upper
        })
    {
        return Err(Error("invalid finalizer input"));
    }
    body.inertia = alternate_inertia;
    let weighted = pass(body, state, wheels, true)?;
    let predicted = pose::predict(body.basis, weighted, delta)?;
    let mut result = pass(body, state, wheels, false)?;
    result.position = predicted[3];
    body.basis = predicted;
    let wheels = wheels.map(|w| {
        let clipped = if w.displacement < w.floor {
            if w.lower > w.floor {
                w.lower
            } else if w.floor > w.clip_upper {
                w.clip_upper
            } else {
                w.floor
            }
        } else {
            w.displacement
        };
        WheelResult {
            displacement: w.displacement,
            speed: w.speed,
            floor_difference: clipped - w.displacement,
            force: w.raw / delta + w.force,
            remaining: [0.; 4],
        }
    });
    if wheels
        .iter()
        .any(|w| !w.floor_difference.is_finite() || !w.force.is_finite())
    {
        return Err(Error("finalized wheel exceeds finite range"));
    }
    Ok(Finalized {
        body,
        state: result,
        wheels,
        clock: 0.,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weighted_pose_and_raw_velocity_are_distinct() {
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
            raw: 2.,
            weighted: 1.,
            contact_raw: 0.,
            contact_weighted: 0.,
            displacement: 0.,
            speed: 0.,
            floor: 0.,
            lower: -1.,
            clip_upper: 1.,
            force: 0.,
        };
        let result = finish(body, state, [wheel; 4], [6., 8., 10.], 0.25).unwrap();
        assert_eq!(result.state.linear, [0., 4., 0., 0.]);
        assert_eq!(result.state.position, [10., 20.5, 30., 1.]);
        assert_eq!(result.body.inertia, [6., 8., 10.]);
        assert_eq!(result.wheels[0].force, 8.);
        assert!(finish(body, state, [wheel; 4], [0.; 3], 0.25).is_err());
    }
}
