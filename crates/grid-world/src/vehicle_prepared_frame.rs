//! Actual post-grip interval through controls, propulsion and selected forces.
use crate::{
    Error, dynamics::ImpulseBody, surface_drag, suspension_prepare::State, suspension_vehicle,
    vehicle_controls, vehicle_force::Frame,
};
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub drag: surface_drag::Material,
    pub roughness: f32,
    pub roll: f32,
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, x: f32) {
    b[o / 4] = x.to_bits();
}
fn vector(b: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, o + i * 4))
}
fn draw(b: &mut [u32]) -> u32 {
    let x = b[0x1644 / 4]
        .wrapping_mul(0x41c64e6d)
        .wrapping_add(0x40584c0f);
    b[0x1644 / 4] = x;
    x % 65535
}

/// Prepared contacts and vectors are inputs; no pose/world frame is advanced.
#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed value-first sums"
)]
pub fn apply(
    frame: &mut Frame,
    pair: [usize; 2],
    materials: [Option<Material>; 4],
    delta: f32,
) -> Result<(), Error> {
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || [0x1648, 0x1f6c, 0x1f7c, 0x17a8, 0x1894]
            .into_iter()
            .any(|o| !f(&frame.body, o).is_finite())
        || materials.iter().enumerate().any(|(i, m)| {
            m.is_some() != frame.material_multipliers[i].is_some()
                || m.is_some_and(|m| !m.roughness.is_finite() || !m.roll.is_finite())
        })
    {
        return Err(Error("invalid post-grip interval inputs"));
    }
    let mut s = frame.clone();
    let b = &s.body;
    let drag = surface_drag::apply(surface_drag::Input {
        body: ImpulseBody {
            mass: f(b, 0x15a0),
            inertia: std::array::from_fn(|i| f(b, 0x15a8 + i * 4)),
            basis: std::array::from_fn(|i| vector(b, 0x15e0 + i * 16)),
        },
        linear: vector(b, 0x15c0),
        angular: vector(b, 0x15d0),
        speed: f(b, 0x1648),
        delta,
        wheels: std::array::from_fn(|i| surface_drag::Wheel {
            lever: vector(&s.view, 0x250 + i * 16),
            spin: f(b, 0x1990 + i * 0x150 + 0x104),
            radius: f(b, 0x1990 + i * 0x150 + 0xe0),
            additive_22b4: f(b, 0x22b4 + i * 4),
            material: materials[i].map(|m| m.drag),
        }),
    })?;
    for i in 0..4 {
        set(&mut s.body, 0x15c0 + i * 4, drag.linear[i]);
        set(&mut s.body, 0x15d0 + i * 4, drag.angular[i]);
        set(&mut s.body, 0x1990 + i * 0x150 + 0x104, drag.spins[i]);
    }
    let b = &mut s.body;
    let speed = f(b, 0x1648);
    let timer = f(b, 0x1f6c);
    if !(10. ..=50.).contains(&speed) || (timer != 0. && timer != 1.) {
        let rate = if speed > 30. { 0.5 } else { -0.5 };
        set(b, 0x1f6c, (rate * delta + timer).clamp(0., 1.));
    }
    let timer = if f(b, 0x17a8) * 0.5 <= f(b, 0x1894) {
        f(b, 0x1f7c) - delta
    } else {
        f(b, 0x1f7c) + delta
    };
    set(b, 0x1f7c, timer.clamp(0., 2.));
    s.solver[0] = 0;
    s.solver[1] = (s.solver[1] & !255) | 1;
    s.solver[2] = 0;
    s.solver[3] = 0;
    let incoming = std::array::from_fn(|i| f(&s.body, 0x15a8 + i * 4));
    let alternate = suspension_vehicle::alternate_inertia(&State { words: s.body }, incoming)?;
    for i in 0..3 {
        set(&mut s.view, 0x48 + i * 4, incoming[i]);
        set(&mut s.body, 0x15a8 + i * 4, alternate[i]);
    }
    let mut roughness = 0.;
    for m in materials.into_iter().flatten() {
        roughness = m.roughness + roughness;
    }
    if roughness > 0. && speed > 8. {
        let magnitude = (roughness / 4.) * ((speed - 8.) * f32::from_bits(0x3daaaaab));
        let threshold = (magnitude * delta) * 3072.;
        if !threshold.is_finite() || !(0. ..=2_000_000_000.).contains(&threshold) {
            return Err(Error("roughness threshold exceeds conversion bound"));
        }
        if ((draw(&mut s.body) & 255) as i32) < threshold as i32 {
            for i in 0..3 {
                let random = (draw(&mut s.body) & 255) as f32;
                let offset = 0x15d0 + i * 4;
                let x = ((((random - 128.) * 0.0078125) * magnitude) * f32::from_bits(0x3d4ccccd))
                    + f(&s.body, offset);
                set(&mut s.body, offset, x);
            }
        }
    }
    for i in 0..4 {
        set(&mut s.view, 0x8c + i * 4, 0.);
    }
    vehicle_controls::apply(&mut s, pair, materials.map(|m| m.map(|m| m.roll)), delta)?;
    *frame = s;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn force_failure_rolls_back_drag_timers_reset_inertia_and_controls() {
        let mut frame = Frame {
            body: [0; crate::vehicle_force::BODY_WORDS],
            view: [0; crate::vehicle_force::VIEW_WORDS],
            solver: [0xdeadbeef; 884],
            driven: vec![2, 3],
            material_multipliers: [None; 4],
            diagnostics: [1.; 8],
        };
        frame.body[0x1598 / 4] = 1;
        frame.body[0x1620 / 4] = 4;
        frame.body[0x196c / 4] = 1;
        frame.body[0xe48 / 4] = 10;
        frame.body[0xe2c / 4] = 2;
        for (o, x) in [
            (0x15a0, 1000.),
            (0x15a8, 100.),
            (0x15ac, 100.),
            (0x15b0, 100.),
            (0x15a4, f32::NAN),
            (0x17a0, 0.22),
            (0x17a4, 900.),
            (0x17a8, 105.),
            (0x1f5c, 1.),
            (0x1f98, 3600.),
            (0x1fa0, 3600.),
            (0x125c, 1.),
            (0x1260, 1.),
            (0xe54, 0.4),
            (0x21d4, 1.6),
            (0x21d8, 2.6),
            (0x2298, 1.),
            (0x229c, 1.),
            (0xe30, 0.2),
            (0x2198, 1.),
            (0x219c, 1.),
            (0x21a0, 1.),
            (0x21a4, 1.),
            (0x21a8, 1.),
            (0x21ac, 1.),
            (0x21b0, 1.),
            (0x21b8, 1.),
            (0x21bc, 2.),
            (0x21c0, 1.),
        ] {
            set(&mut frame.body, o, x);
        }
        for i in 1..=9 {
            set(&mut frame.body, 0x18f4 + i * 4, 0.5);
        }
        for i in 0..4 {
            set(&mut frame.body, 0x1990 + i * 0x150 + 0xf0, 1.);
            set(&mut frame.body, 0x1990 + i * 0x150 + 0xf4, 1.);
            set(&mut frame.body, 0x1990 + i * 0x150 + 0xe0, 0.5);
        }
        let before = frame.clone();
        let error = apply(&mut frame, [3, 2], [None; 4], 0.01).unwrap_err();
        assert_eq!(error.0, "nonfinite or invalid physical force inputs");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
}
