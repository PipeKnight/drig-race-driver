//! Four-active-wheel substep with extra tyre axes inactive.
use crate::{Error, suspension};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub suspension: suspension::Wheel,
    pub axis: [f32; 4],
    pub body_speed: f32,
    pub body_rate: f32,
    pub floor: f32,
    pub contact_speed: f32,
    pub raw: f32,
    pub weighted: f32,
    pub contact_raw: f32,
    pub contact_weighted: f32,
    pub contact: bool,
    pub lower: f32,
    pub upper: f32,
}

/// Supplied prepared response matrices and travel bounds. No extra tyre impulses.
/// The +/-99 envelope is a diagnostic restriction, not a game clamp.
#[allow(clippy::assign_op_pattern)] // Keep the observed f32 operand order.
pub fn step(
    mut wheels: [Wheel; 4],
    response: [[f32; 4]; 4],
    secondary: [[f32; 4]; 4],
    scale: f32,
    delta: f32,
    weight: f32,
) -> Result<[Wheel; 4], Error> {
    if !weight.is_finite()
        || response
            .into_iter()
            .flatten()
            .chain(secondary.into_iter().flatten())
            .any(|v| !v.is_finite())
        || wheels.iter().any(|w| {
            w.axis
                .into_iter()
                .chain([
                    w.body_speed,
                    w.body_rate,
                    w.floor,
                    w.contact_speed,
                    w.raw,
                    w.weighted,
                    w.contact_raw,
                    w.contact_weighted,
                    w.lower,
                    w.upper,
                ])
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid wheel step inputs"));
    }
    if wheels
        .iter()
        .any(|w| w.lower > w.upper || w.lower.abs() >= 99. || w.upper.abs() >= 99.)
    {
        return Err(Error("invalid supported wheel travel bounds"));
    }
    let impulses = suspension::impulses(wheels.map(|w| w.suspension), scale, delta)?;
    let inverse_wheel_mass = f32::from_bits(0x3d08_8889);
    let gravity = [0., f32::from_bits(0xc11c_f5c3), 0., 0.];
    for wheel in &mut wheels {
        let change = delta * wheel.body_rate;
        wheel.body_speed += change;
        wheel.suspension.speed -= change;
        let axis = wheel.axis;
        let acceleration = (axis[3] * gravity[3] + axis[2] * gravity[2])
            + (axis[1] * gravity[1] + axis[0] * gravity[0]);
        wheel.suspension.speed = acceleration * delta + wheel.suspension.speed;
    }
    for initiator in 0..4 {
        let impulse = impulses[initiator];
        wheels[initiator].raw = impulse + wheels[initiator].raw;
        wheels[initiator].weighted = impulse * weight + wheels[initiator].weighted;
        for (receiver, wheel) in wheels.iter_mut().enumerate() {
            let change = impulse * response[initiator][receiver];
            wheel.body_speed += change;
            wheel.suspension.speed -= change;
        }
        wheels[initiator].suspension.speed -= impulse * inverse_wheel_mass;
    }
    for initiator in 0..4 {
        let wheel = &mut wheels[initiator];
        wheel.suspension.displacement =
            wheel.suspension.speed * delta + wheel.suspension.displacement;
        wheel.floor = (wheel.floor - delta * wheel.body_speed) + delta * wheel.contact_speed;
        if wheel.suspension.displacement > wheel.upper {
            wheel.suspension.displacement = wheel.upper;
            if wheel.suspension.speed > 0. {
                stop(&mut wheels, response, initiator, weight, inverse_wheel_mass)?;
            }
        }
        let wheel = &mut wheels[initiator];
        if wheel.suspension.displacement < wheel.lower {
            wheel.suspension.displacement = wheel.lower;
            if wheel.suspension.speed < 0. {
                stop(&mut wheels, response, initiator, weight, inverse_wheel_mass)?;
            }
        }
        let wheel = &mut wheels[initiator];
        if wheel.suspension.displacement <= wheel.floor {
            wheel.suspension.displacement = wheel.floor;
            wheel.contact = true;
            let target = wheel.contact_speed - wheel.body_speed;
            if wheel.suspension.speed < target {
                let denominator = secondary[initiator][initiator] + inverse_wheel_mass;
                if denominator == 0. || !denominator.is_finite() {
                    return Err(Error("singular wheel contact response"));
                }
                let impulse = (target - wheel.suspension.speed) / denominator;
                wheel.suspension.speed = impulse * inverse_wheel_mass + wheel.suspension.speed;
                wheel.contact_raw = impulse + wheel.contact_raw;
                wheel.contact_weighted = impulse * weight + wheel.contact_weighted;
                for (receiver, other) in wheels.iter_mut().enumerate() {
                    other.contact_speed -= secondary[initiator][receiver] * impulse;
                }
            }
        }
    }
    if wheels.iter().any(|w| {
        w.suspension.displacement.abs() >= 99.
            || [
                w.suspension.displacement,
                w.suspension.speed,
                w.body_speed,
                w.floor,
                w.contact_speed,
                w.raw,
                w.weighted,
                w.contact_raw,
                w.contact_weighted,
            ]
            .into_iter()
            .any(|v| !v.is_finite())
    }) {
        return Err(Error("wheel step exceeds supported finite envelope"));
    }
    Ok(wheels)
}

#[allow(clippy::assign_op_pattern)] // Keep the observed weighted-impulse operand order.
fn stop(
    wheels: &mut [Wheel; 4],
    response: [[f32; 4]; 4],
    initiator: usize,
    weight: f32,
    inverse_wheel_mass: f32,
) -> Result<(), Error> {
    let denominator = response[initiator][initiator] + inverse_wheel_mass;
    if denominator == 0. || !denominator.is_finite() {
        return Err(Error("singular wheel travel-stop response"));
    }
    let impulse = wheels[initiator].suspension.speed / denominator;
    wheels[initiator].raw += impulse;
    wheels[initiator].weighted = impulse * weight + wheels[initiator].weighted;
    for (receiver, wheel) in wheels.iter_mut().enumerate() {
        let change = response[initiator][receiver] * impulse;
        wheel.body_speed += change;
        wheel.suspension.speed -= change;
    }
    wheels[initiator].suspension.speed = 0.;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floor_contact_balances_free_wheel_gravity() {
        let wheel = Wheel {
            suspension: suspension::Wheel {
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
            axis: [0., 1., 0., 0.],
            body_speed: 0.,
            body_rate: 0.,
            floor: 0.,
            contact_speed: 0.,
            raw: 0.,
            weighted: 0.,
            contact_raw: 0.,
            contact_weighted: 0.,
            contact: false,
            lower: -98.,
            upper: 98.,
        };
        let result = step([wheel; 4], [[0.; 4]; 4], [[0.; 4]; 4], 1., 0.001, 1.).unwrap();
        for wheel in result {
            assert!(wheel.contact);
            assert_eq!(wheel.suspension.displacement, 0.);
            assert!(wheel.suspension.speed.abs() < 1e-8);
            assert!((wheel.contact_raw - 0.2943).abs() < 1e-6);
        }
    }
}
