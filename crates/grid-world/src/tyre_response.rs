//! Apply supplied active tyre impulses and their prepared cross-wheel responses.
use crate::{Error, tyre_friction, wheel_step};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    /// State after suspension/gravity/normal-contact processing.
    pub normal: wheel_step::Wheel,
    /// Slip inputs before this tail; normal impulse and delta supplied by caller.
    pub tyre: tyre_friction::Input,
    pub raw: [f32; 2],
    pub weighted: [f32; 2],
}

/// Axis order: suspension, first tyre, second tyre. All tyre impulses are
/// calculated before applying any response. Suspension impulses are the
/// precomputed spring/damper contributions, excluding travel-stop impulses.
#[allow(clippy::assign_op_pattern)] // Retain original accumulation order.
pub fn apply(
    mut wheels: [Wheel; 4],
    response: [[[[f32; 4]; 4]; 3]; 3],
    suspension: [f32; 4],
    weight: f32,
) -> Result<[Wheel; 4], Error> {
    if !weight.is_finite()
        || response
            .iter()
            .flatten()
            .flatten()
            .flatten()
            .chain(&suspension)
            .any(|v| !v.is_finite())
        || wheels
            .iter()
            .flat_map(|w| w.raw.into_iter().chain(w.weighted))
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid tyre response inputs"));
    }
    let mut outputs = Vec::with_capacity(4);
    for wheel in &mut wheels {
        let output = tyre_friction::resolve(wheel.tyre)?;
        wheel.tyre.displacement = output.displacement;
        outputs.push(output);
    }
    for initiator in 0..4 {
        let first = outputs[initiator].first_impulse;
        let second = outputs[initiator].second_impulse;
        wheels[initiator].raw[0] = wheels[initiator].raw[0] + first;
        wheels[initiator].weighted[0] = first * weight + wheels[initiator].weighted[0];
        wheels[initiator].raw[1] = wheels[initiator].raw[1] + second;
        wheels[initiator].weighted[1] = second * weight + wheels[initiator].weighted[1];
        for (receiver, wheel) in wheels.iter_mut().enumerate() {
            let change = response[2][0][initiator][receiver] * second
                + response[1][0][initiator][receiver] * first;
            wheel.normal.body_speed = change + wheel.normal.body_speed;
            wheel.normal.suspension.speed = wheel.normal.suspension.speed - change;
            wheel.tyre.lateral = wheel.tyre.lateral
                + ((response[0][1][initiator][receiver] * suspension[initiator]
                    + response[2][1][initiator][receiver] * second)
                    + response[1][1][initiator][receiver] * first);
            wheel.tyre.rate = ((response[0][2][initiator][receiver] * suspension[initiator]
                + response[2][2][initiator][receiver] * second)
                + response[1][2][initiator][receiver] * first)
                + wheel.tyre.rate;
        }
        let angular = outputs[initiator].angular_delta;
        wheels[initiator].tyre.spin = angular + wheels[initiator].tyre.spin;
        wheels[initiator].tyre.rate = wheels[initiator].tyre.rate - angular;
    }
    if wheels
        .iter()
        .flat_map(|w| {
            w.raw.into_iter().chain(w.weighted).chain([
                w.normal.body_speed,
                w.normal.suspension.speed,
                w.tyre.lateral,
                w.tyre.rate,
                w.tyre.spin,
            ])
        })
        .any(|v| !v.is_finite())
    {
        return Err(Error("tyre response exceeds finite range"));
    }
    Ok(wheels)
}
