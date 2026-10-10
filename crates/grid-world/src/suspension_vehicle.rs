//! Complete selected suspension wrapper: preparation, substeps and publication.
//! The source-selected extra tyre-axis branch is disabled; later forces are separate.
use crate::{
    Error, dynamics, finalize, suspension,
    suspension_prepare::{self, Contact, State},
    wheel_step,
};
const C: usize = 0x3d0;

#[allow(clippy::assign_op_pattern)] // Retain the observed multiplication operand order.
fn alternate_inertia(s: &State, incoming: [f32; 3]) -> Result<[f32; 3], Error> {
    let low = s.f(0x21b4);
    let high = s.f(0x21b8);
    let top = s.f(0x21bc);
    if !(low < high && high < top) {
        return Err(Error("invalid inertia speed intervals"));
    }
    let t = ((s.f(0x1648) - low) / (high - low)).clamp(0., 1.);
    let base_y = s.f(0x2198);
    let middle: [f32; 3] = std::array::from_fn(|i| s.f(0x219c + i * 4));
    let upper: [f32; 3] = std::array::from_fn(|i| s.f(0x21a8 + i * 4));
    let mut result = [
        ((middle[0] - 1.) * t + 1.) * incoming[0],
        ((((middle[1] - base_y) * t + base_y) - 1.) * t + 1.) * incoming[1],
        ((middle[2] - 1.) * t + 1.) * incoming[2],
    ];
    // The actual instructions compare normalized t with these stored intervals
    // again. Do not silently replace t by raw speed in the second stage.
    for (i, value) in result.iter_mut().enumerate() {
        let base = if i == 1 { base_y } else { 1. };
        let scale = if t < low {
            base
        } else if t < high {
            let fraction = (t - low) / (high - low);
            (middle[i] - base) * fraction * fraction + base
        } else if t < top {
            let fraction = (t - high) / (top - high);
            (upper[i] - middle[i]) * fraction * fraction + middle[i]
        } else {
            upper[i]
        };
        *value = scale * *value;
    }
    result[2] = s.f(0x21c0) * result[2];
    if result.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(Error("invalid alternate inertia"));
    }
    Ok(result)
}

/// Execute the whole supported wrapper and return the saved force-view inertia.
/// Geometry and contacts are recomputed; all owned state commits on success.
pub fn apply(
    state: &mut State,
    delta: f32,
    mut query: impl FnMut(usize, crate::contact_ray::Ray) -> Result<Option<Contact>, Error>,
) -> Result<[f32; 3], Error> {
    let mut s = state.clone();
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || !(1. / delta).is_finite()
        || s.words[(C + 0x70c) / 4] & 255 != 0
    {
        return Err(Error("unsupported suspension vehicle delta/branch"));
    }
    for off in [C, 0x11e8, 0x185c]
        .into_iter()
        .chain((0..11).map(|i| 0x2198 + i * 4))
    {
        if !s.f(off).is_finite() {
            return Err(Error("nonfinite suspension vehicle input"));
        }
    }
    if s.f(C) <= 0. {
        return Err(Error("invalid suspension vehicle scale"));
    }
    if state.words[0x2278 / 4] != 0 {
        return Err(Error("disabled suspension wheels unsupported"));
    }
    let incoming: [f32; 3] = std::array::from_fn(|i| s.f(0x15a8 + i * 4));
    if incoming.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(Error("invalid vehicle inertia"));
    }
    s.set(C + 0x708, (1. - s.f(0x185c)) * s.f(0x11e8) + s.f(0x185c));
    let alternate = alternate_inertia(&s, incoming)?;
    for i in 0..3 {
        s.set(C + 0x6f0 + i * 4, incoming[i]);
        s.set(C + 0x6fc + i * 4, alternate[i]);
    }
    let mut contacts = [None; 4];
    suspension_prepare::apply(&mut s, delta, |i, ray| {
        let hit = query(i, ray)?;
        contacts[i] = hit;
        Ok(hit)
    })?;
    let mut wheels = std::array::from_fn(|i| {
        let c = C + i * 0x170;
        let w = 0x1990 + i * 0x150;
        wheel_step::Wheel {
            suspension: suspension::Wheel {
                displacement: s.f(c + 0x64),
                speed: s.f(c + 0x74),
                spring: s.f(c + 0x60),
                preload: s.f(w + 0x54),
                travel_denominator: s.f(w + 0x4c),
                damper: s.f(w + 0x60),
                speed_threshold: s.f(w + 0x64),
                high_speed_ratio: s.f(w + 0x68),
                negative_speed_scale: s.f(w + 0x114),
                bump: s.f(w + 0x58),
                axle_coupling: s.f(w + 0x70),
            },
            axis: s.vector(c + 0x30),
            body_speed: s.f(c + 0x70),
            body_rate: s.f(C + 0x640 + i * 4),
            floor: s.f(c + 0x68),
            contact_speed: s.f(c + 0x78),
            raw: s.f(c + 0x168),
            weighted: s.f(c + 0x16c),
            contact_raw: s.f(c + 0x174),
            contact_weighted: s.f(c + 0x170),
            contact: false,
            lower: s.f(w + 0x48),
            upper: s.f(w + 0x50),
        }
    });
    let response = std::array::from_fn(|i| s.vector(C + i * 0x170 + 0x88));
    let secondary = s.basis(C + 0x650);
    let schedule = dynamics::substeps(delta)?;
    let mut weight = 1.;
    for _ in 0..schedule.count {
        wheels = wheel_step::step(wheels, response, secondary, s.f(C), schedule.delta, weight)?;
        weight -= schedule.weight_decrement;
    }
    for (i, wheel) in wheels.iter().enumerate() {
        let c = C + i * 0x170;
        for (offset, value) in [
            (0x64, wheel.suspension.displacement),
            (0x74, wheel.suspension.speed),
            (0x70, wheel.body_speed),
            (0x68, wheel.floor),
            (0x78, wheel.contact_speed),
            (0x168, wheel.raw),
            (0x16c, wheel.weighted),
            (0x174, wheel.contact_raw),
            (0x170, wheel.contact_weighted),
        ] {
            s.set(c + offset, value);
        }
        s.words[(c + 0x84) / 4] = (s.words[(c + 0x84) / 4] & !255) | u32::from(wheel.contact);
    }
    let mut material_real = [1.; 4];
    for i in 0..4 {
        if s.words[(0x1990 + i * 0x150 + 0xc0) / 4] != 0 {
            material_real[i] = contacts[i]
                .ok_or(Error("missing prepared contact mechanics"))?
                .material
                .real_proportion;
        }
    }
    let final_wheels = std::array::from_fn(|i| {
        let c = C + i * 0x170;
        let w = 0x1990 + i * 0x150;
        let wheel = wheels[i];
        let floor = if s.words[(w + 0xc0) / 4] != 0 {
            (1. - material_real[i]) * s.f(c + 0x80) + wheel.floor
        } else {
            wheel.floor
        };
        finalize::Wheel {
            point: s.vector(c + 0x10),
            axis: wheel.axis,
            projection_axis: s.vector(c + 0x40),
            raw: wheel.raw,
            weighted: wheel.weighted,
            contact_raw: wheel.contact_raw,
            contact_weighted: wheel.contact_weighted,
            displacement: wheel.suspension.displacement,
            speed: wheel.suspension.speed,
            floor,
            lower: wheel.lower,
            clip_upper: s.f(w + 0x4c),
            force: s.f(w + 0x138),
        }
    });
    let body = dynamics::ImpulseBody {
        mass: s.f(0x15a0),
        inertia: incoming,
        basis: s.basis(0x15e0),
    };
    let physical = dynamics::BodyState {
        linear: s.vector(0x15c0),
        angular: s.vector(0x15d0),
        position: body.basis[3],
    };
    let finished = finalize::finish(body, physical, final_wheels, incoming, delta)?;
    let physical = dynamics::apply_solver_contribution(
        finished.state,
        delta,
        s.vector(C + 0x620),
        s.vector(C + 0x630),
    )?;
    for (i, row) in finished.body.basis.into_iter().enumerate() {
        s.put(0x15e0 + i * 16, row);
    }
    s.put(0x15c0, physical.linear);
    s.put(0x15d0, physical.angular);
    s.put(0x1610, physical.position);
    for (i, result) in finished.wheels.into_iter().enumerate() {
        let c = C + i * 0x170;
        let w = 0x1990 + i * 0x150;
        for (offset, value) in [
            (0x40, result.displacement),
            (0x3c, result.speed),
            (0x44, result.floor_difference),
            (0x138, result.force),
        ] {
            s.set(w + offset, value);
        }
        for offset in [0x140, 0x144, 0x148, 0x14c, 0x168, 0x16c, 0x170, 0x174] {
            s.set(c + offset, 0.);
        }
        if !wheels[i].contact {
            s.words[(w + 0xc0) / 4] = 0;
        }
        if result.force == 0. {
            s.set(c + 0x150, 0.);
        }
        s.set(c + 0x7c, s.f(c + 0x80));
    }
    s.set(C + 0x610, 0.);
    for i in 0..4 {
        s.put(C + 0x690 + i * 16, s.vector(0x15e0 + i * 16));
    }
    s.put(C + 0x6d0, physical.linear);
    s.put(C + 0x6e0, physical.angular);
    s.words[(C + 0x5d0) / 4] = (s.words[(C + 0x5d0) / 4] & !255) | 1;
    *state = s;
    Ok(incoming)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_world_query_rolls_back_inertia_and_wrapper_setup() {
        let mut state = State {
            words: [0; suspension_prepare::WORDS],
        };
        state.words[(C + 0x5d8) / 4] = 4;
        state.set(C, 1.);
        state.set(0x11e8, 0.7);
        state.set(0x185c, 0.5);
        state.set(0x15a0, 1000.);
        for i in 0..3 {
            state.set(0x15a8 + i * 4, 600.);
        }
        for i in 0..7 {
            state.set(0x2198 + i * 4, 1.);
        }
        for (offset, value) in [(0x21b4, 2.), (0x21b8, 5.), (0x21bc, 10.), (0x21c0, 1.)] {
            state.set(offset, value);
        }
        let basis = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 1., 0., 1.],
        ];
        for (i, row) in basis.into_iter().enumerate() {
            state.put(0x15e0 + i * 16, row);
        }
        for i in 0..4 {
            let c = C + i * 0x170;
            let w = 0x1990 + i * 0x150;
            state.put(c + 0x50, [0., 1., 0., 0.]);
            for (offset, value) in [(0x48, -0.1), (0x4c, 0.2), (0x50, 0.25), (0xe0, 0.3)] {
                state.set(w + offset, value);
            }
            state.set(c + 0x6c, -1000.);
        }
        let before = state.words;
        let mut calls = 0;
        assert!(
            apply(&mut state, 0.01, |_, _| {
                calls += 1;
                Err(Error("authored query failure"))
            })
            .is_err()
        );
        assert_eq!(calls, 1);
        assert_eq!(state.words, before);
        state.words[0x2278 / 4] = 0x0100;
        let disabled = state.words;
        assert!(apply(&mut state, 0.01, |_, _| panic!("disabled-wheel query")).is_err());
        assert_eq!(state.words, disabled);
    }
}
